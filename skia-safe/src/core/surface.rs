#[cfg(feature = "gpu")]
use crate::gpu;
use crate::{
    prelude::*, Bitmap, Canvas, DeferredDisplayList, IPoint, IRect, ISize, IVector, Image,
    ImageInfo, Paint, Pixmap, Point, SamplingOptions, SurfaceCharacterization, SurfaceProps,
};
use skia_bindings::{self as sb, SkRefCntBase, SkSurface};
use std::{fmt, ptr};

/// ContentChangeMode members are parameters to [`Surface::notify_content_will_change()`].
pub use skia_bindings::SkSurface_ContentChangeMode as ContentChangeMode;
variant_name!(ContentChangeMode::Retain);

#[cfg(feature = "gpu")]
pub use skia_bindings::SkSurface_BackendHandleAccess as BackendHandleAccess;
#[cfg(feature = "gpu")]
variant_name!(BackendHandleAccess::FlushWrite);

pub use skia_bindings::SkSurface_BackendSurfaceAccess as BackendSurfaceAccess;
variant_name!(BackendSurfaceAccess::Present);

/// [`Surface`] is responsible for managing the pixels that a canvas draws into. The pixels can be
/// allocated either in CPU memory (a raster surface) or on the GPU (a `RenderTarget` surface).
/// [`Surface`] takes care of allocating a [`Canvas`] that will draw into the surface. Call
/// `surface_get_canvas()` to use that canvas (but don't delete it, it is owned by the surface).
/// [`Surface`] always has non-zero dimensions. If there is a request for a new surface, and either
/// of the requested dimensions are zero, then `None` will be returned.
pub type Surface = RCHandle<SkSurface>;
require_type_equality!(sb::SkSurface_INHERITED, sb::SkRefCnt);

impl NativeRefCountedBase for SkSurface {
    type Base = SkRefCntBase;
}

impl fmt::Debug for Surface {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Surface")
            // self must be mutable (this goes through Canvas).
            // .field("image_info", &self.image_info())
            // .field("generation_id", &self.generation_id())
            .field("props", &self.props())
            .finish()
    }
}

impl Surface {
    /// Allocates raster [`Surface`]. [`Canvas`] returned by [`Surface`] draws directly into pixels.
    ///
    /// [`Surface`] is returned if all parameters are valid.
    /// Valid parameters include:
    /// info dimensions are greater than zero;
    /// info contains [`crate::ColorType`] and [`crate::AlphaType`] supported by raster surface;
    /// pixels is not `None`;
    /// `row_bytes` is large enough to contain info width pixels of [`crate::ColorType`].
    ///
    /// Pixel buffer size should be info height times computed `row_bytes`.
    /// Pixels are not initialized.
    /// To access pixels after drawing, [`Self::peek_pixels()`] or [`Self::read_pixels()`].
    ///
    /// * `image_info` - width, height, [`crate::ColorType`], [`crate::AlphaType`], [`crate::ColorSpace`],
    ///                      of raster surface; width and height must be greater than zero
    /// * `pixels` - pointer to destination pixels buffer
    /// * `row_bytes` - interval from one [`Surface`] row to the next
    /// * `surface_props` - LCD striping orientation and setting for device independent fonts;
    ///                      may be `None`
    /// Returns: [`Surface`] if all parameters are valid; otherwise, `None`
    pub fn new_raster_direct<'pixels>(
        image_info: &ImageInfo,
        pixels: &'pixels mut [u8],
        row_bytes: impl Into<Option<usize>>,
        surface_props: Option<&SurfaceProps>,
    ) -> Option<Borrows<'pixels, Surface>> {
        let row_bytes = row_bytes
            .into()
            .unwrap_or_else(|| image_info.min_row_bytes());

        if pixels.len() < image_info.compute_byte_size(row_bytes) {
            return None;
        };

        Self::from_ptr(unsafe {
            sb::C_SkSurface_MakeRasterDirect(
                image_info.native(),
                pixels.as_mut_ptr() as _,
                row_bytes,
                surface_props.native_ptr_or_null(),
            )
        })
        .map(move |surface| surface.borrows(pixels))
    }

    // TODO: MakeRasterDirect(&Pixmap)
    // TODO: MakeRasterDirectReleaseProc()?

    /// Allocates raster [`Surface`]. [`Canvas`] returned by [`Surface`] draws directly into pixels.
    /// Allocates and zeroes pixel memory. Pixel memory size is `image_info.height()` times
    /// `row_bytes`, or times `image_info.min_row_bytes()` if `row_bytes` is zero.
    /// Pixel memory is deleted when [`Surface`] is deleted.
    ///
    /// [`Surface`] is returned if all parameters are valid.
    /// Valid parameters include:
    /// info dimensions are greater than zero;
    /// info contains [`crate::ColorType`] and [`crate::AlphaType`] supported by raster surface;
    /// `row_bytes` is large enough to contain info width pixels of [`crate::ColorType`], or is zero.
    ///
    /// If `row_bytes` is zero, a suitable value will be chosen internally.
    ///
    /// * `image_info` - width, height, [`crate::ColorType`], [`crate::AlphaType`], [`crate::ColorSpace`],
    ///                      of raster surface; width and height must be greater than zero
    /// * `row_bytes` - interval from one [`Surface`] row to the next; may be zero
    /// * `surface_props` - LCD striping orientation and setting for device independent fonts;
    ///                      may be `None`
    /// Returns: [`Surface`] if all parameters are valid; otherwise, `None`
    pub fn new_raster(
        image_info: &ImageInfo,
        row_bytes: impl Into<Option<usize>>,
        surface_props: Option<&SurfaceProps>,
    ) -> Option<Self> {
        Self::from_ptr(unsafe {
            sb::C_SkSurface_MakeRaster(
                image_info.native(),
                row_bytes.into().unwrap_or_default(),
                surface_props.native_ptr_or_null(),
            )
        })
    }

    /// Allocates raster [`Surface`]. [`Canvas`] returned by [`Surface`] draws directly into pixels.
    /// Allocates and zeroes pixel memory. Pixel memory size is height times width times
    /// four. Pixel memory is deleted when [`Surface`] is deleted.
    ///
    /// Internally, sets [`ImageInfo`] to width, height, native color type, and
    /// [`crate::AlphaType::Premul`].
    ///
    /// [`Surface`] is returned if width and height are greater than zero.
    ///
    /// Use to create [`Surface`] that matches [`crate::PMColor`], the native pixel arrangement on
    /// the platform. [`Surface`] drawn to output device skips converting its pixel format.
    ///
    /// * `width` - pixel column count; must be greater than zero
    /// * `height` - pixel row count; must be greater than zero
    /// * `surface_props` - LCD striping orientation and setting for device independent
    ///                      fonts; may be `None`
    /// Returns: [`Surface`] if all parameters are valid; otherwise, `None`
    pub fn new_raster_n32_premul(size: impl Into<ISize>) -> Option<Self> {
        let size = size.into();
        Self::from_ptr(unsafe {
            sb::C_SkSurface_MakeRasterN32Premul(size.width, size.height, ptr::null())
        })
    }
}

#[cfg(feature = "gpu")]
impl Surface {
    /// Wraps a GPU-backed texture into [`Surface`]. Caller must ensure the texture is
    /// valid for the lifetime of returned [`Surface`]. If `sample_cnt` greater than zero,
    /// creates an intermediate MSAA [`Surface`] which is used for drawing `backend_texture`.
    ///
    /// [`Surface`] is returned if all parameters are valid. `backend_texture` is valid if
    /// its pixel configuration agrees with `color_space` and context; for instance, if
    /// `backend_texture` has an sRGB configuration, then context must support sRGB,
    /// and `color_space` must be present. Further, `backend_texture` width and height must
    /// not exceed context capabilities, and the context must be able to support
    /// back-end textures.
    ///
    /// * `context` - GPU context
    /// * `backend_texture` - texture residing on GPU
    /// * `sample_cnt` - samples per pixel, or 0 to disable full scene anti-aliasing
    /// * `color_space` - range of colors; may be `None`
    /// * `surface_props` - LCD striping orientation and setting for device independent
    ///                            fonts; may be `None`
    /// Returns: [`Surface`] if all parameters are valid; otherwise, `None`
    pub fn from_backend_texture(
        context: &mut gpu::RecordingContext,
        backend_texture: &gpu::BackendTexture,
        origin: gpu::SurfaceOrigin,
        sample_cnt: impl Into<Option<usize>>,
        color_type: crate::ColorType,
        color_space: impl Into<Option<crate::ColorSpace>>,
        surface_props: Option<&SurfaceProps>,
    ) -> Option<Self> {
        Self::from_ptr(unsafe {
            sb::C_SkSurface_MakeFromBackendTexture(
                context.native_mut(),
                backend_texture.native(),
                origin,
                sample_cnt.into().unwrap_or(0).try_into().unwrap(),
                color_type.into_native(),
                color_space.into().into_ptr_or_null(),
                surface_props.native_ptr_or_null(),
            )
        })
    }

    /// Wraps a GPU-backed buffer into [`Surface`]. Caller must ensure `backend_render_target`
    /// is valid for the lifetime of returned [`Surface`].
    ///
    /// [`Surface`] is returned if all parameters are valid. `backend_render_target` is valid if
    /// its pixel configuration agrees with `color_space` and context; for instance, if
    /// `backend_render_target` has an sRGB configuration, then context must support sRGB,
    /// and `color_space` must be present. Further, `backend_render_target` width and height must
    /// not exceed context capabilities, and the context must be able to support
    /// back-end render targets.
    ///
    /// * `context` - GPU context
    /// * `backend_render_target` - GPU intermediate memory buffer
    /// * `color_space` - range of colors
    /// * `surface_props` - LCD striping orientation and setting for device independent
    ///                                 fonts; may be `None`
    /// Returns: [`Surface`] if all parameters are valid; otherwise, `None`
    pub fn from_backend_render_target(
        context: &mut gpu::RecordingContext,
        backend_render_target: &gpu::BackendRenderTarget,
        origin: gpu::SurfaceOrigin,
        color_type: crate::ColorType,
        color_space: impl Into<Option<crate::ColorSpace>>,
        surface_props: Option<&SurfaceProps>,
    ) -> Option<Self> {
        Self::from_ptr(unsafe {
            sb::C_SkSurface_MakeFromBackendRenderTarget(
                context.native_mut(),
                backend_render_target.native(),
                origin,
                color_type.into_native(),
                color_space.into().into_ptr_or_null(),
                surface_props.native_ptr_or_null(),
            )
        })
    }

    /// Returns [`Surface`] on GPU indicated by context. Allocates memory for
    /// pixels, based on the width, height, and [`crate::ColorType`] in [`ImageInfo`].  budgeted
    /// selects whether allocation for pixels is tracked by context. `image_info`
    /// describes the pixel format in [`crate::ColorType`], and transparency in
    /// [`crate::AlphaType`], and color matching in [`crate::ColorSpace`].
    ///
    /// `sample_count` requests the number of samples per pixel.
    /// Pass zero to disable multi-sample anti-aliasing.  The request is rounded
    /// up to the next supported count, or rounded down if it is larger than the
    /// maximum supported count.
    ///
    /// `surface_origin` pins either the top-left or the bottom-left corner to the origin.
    ///
    /// `should_create_with_mips` hints that [`Image`] returned by [`Image::image_snapshot`] is mip map.
    ///
    /// * `context` - GPU context
    /// * `image_info` - width, height, [`crate::ColorType`], [`crate::AlphaType`], [`crate::ColorSpace`];
    ///                              width, or height, or both, may be zero
    /// * `sample_count` - samples per pixel, or 0 to disable full scene anti-aliasing
    /// * `surface_props` - LCD striping orientation and setting for device independent
    ///                              fonts; may be `None`
    /// * `should_create_with_mips` - hint that [`Surface`] will host mip map images
    /// Returns: [`Surface`] if all parameters are valid; otherwise, `None`
    pub fn new_render_target(
        context: &mut gpu::RecordingContext,
        budgeted: gpu::Budgeted,
        image_info: &ImageInfo,
        sample_count: impl Into<Option<usize>>,
        surface_origin: impl Into<Option<gpu::SurfaceOrigin>>,
        surface_props: Option<&SurfaceProps>,
        should_create_with_mips: impl Into<Option<bool>>,
    ) -> Option<Self> {
        Self::from_ptr(unsafe {
            sb::C_SkSurface_MakeRenderTarget(
                context.native_mut(),
                budgeted.into_native(),
                image_info.native(),
                sample_count.into().unwrap_or(0).try_into().unwrap(),
                surface_origin
                    .into()
                    .unwrap_or(gpu::SurfaceOrigin::BottomLeft),
                surface_props.native_ptr_or_null(),
                should_create_with_mips.into().unwrap_or_default(),
            )
        })
    }

    /// Returns [`Surface`] on GPU indicated by context that is compatible with the provided
    /// characterization. budgeted selects whether allocation for pixels is tracked by context.
    ///
    /// * `context` - GPU context
    /// * `characterization` - description of the desired [`Surface`]
    /// Returns: [`Surface`] if all parameters are valid; otherwise, `None`
    pub fn new_render_target_with_characterization(
        context: &mut gpu::RecordingContext,
        characterization: &SurfaceCharacterization,
        budgeted: gpu::Budgeted,
    ) -> Option<Self> {
        Self::from_ptr(unsafe {
            sb::C_SkSurface_MakeRenderTarget2(
                context.native_mut(),
                characterization.native(),
                budgeted.into_native(),
            )
        })
    }

    /// Creates [`Surface`] from CAMetalLayer.
    /// Returned [`Surface`] takes a reference on the CAMetalLayer. The ref on the layer will be
    /// released when the [`Surface`] is destroyed.
    ///
    /// Only available when Metal API is enabled.
    ///
    /// Will grab the current drawable from the layer and use its texture as a `backend_rt` to
    /// create a renderable surface.
    ///
    /// * `context` - GPU context
    /// * `layer` - [`gpu::mtl::Handle`] (expected to be a CAMetalLayer*)
    /// * `sample_cnt` - samples per pixel, or 0 to disable full scene anti-aliasing
    /// * `color_space` - range of colors; may be `None`
    /// * `surface_props` - LCD striping orientation and setting for device independent
    ///                        fonts; may be `None`
    /// * `drawable` - Pointer to drawable to be filled in when this surface is
    ///                        instantiated; may not be `None`
    /// Returns: created [`Surface`], or `None`
    #[allow(clippy::missing_safety_doc)]
    #[allow(clippy::too_many_arguments)]
    #[cfg(feature = "metal")]
    pub unsafe fn from_ca_metal_layer(
        context: &mut gpu::RecordingContext,
        layer: gpu::mtl::Handle,
        origin: gpu::SurfaceOrigin,
        sample_cnt: impl Into<Option<usize>>,
        color_type: crate::ColorType,
        color_space: impl Into<Option<crate::ColorSpace>>,
        surface_props: Option<&SurfaceProps>,
        drawable: *mut gpu::mtl::Handle,
    ) -> Option<Self> {
        Self::from_ptr(sb::C_SkSurface_MakeFromCAMetalLayer(
            context.native_mut(),
            layer,
            origin,
            sample_cnt.into().unwrap_or(0).try_into().unwrap(),
            color_type.into_native(),
            color_space.into().into_ptr_or_null(),
            surface_props.native_ptr_or_null(),
            drawable,
        ))
    }

    #[allow(clippy::missing_safety_doc)]
    #[cfg(feature = "metal")]
    #[deprecated(since = "0.36.0", note = "use from_mtk_view()")]
    pub unsafe fn from_ca_mtk_view(
        context: &mut gpu::DirectContext,
        mtk_view: gpu::mtl::Handle,
        origin: gpu::SurfaceOrigin,
        sample_count: impl Into<Option<usize>>,
        color_type: crate::ColorType,
        color_space: impl Into<Option<crate::ColorSpace>>,
        surface_props: Option<&SurfaceProps>,
    ) -> Option<Self> {
        Self::from_mtk_view(
            context,
            mtk_view,
            origin,
            sample_count,
            color_type,
            color_space,
            surface_props,
        )
    }

    /// Creates [`Surface`] from MTKView.
    /// Returned [`Surface`] takes a reference on the `MTKView`. The ref on the layer will be
    /// released when the [`Surface`] is destroyed.
    ///
    /// Only available when Metal API is enabled.
    ///
    /// Will grab the current drawable from the layer and use its texture as a `backend_rt` to
    /// create a renderable surface.
    ///
    /// * `context` - GPU context
    /// * `layer` - [`gpu::mtl::Handle`] (expected to be a `MTKView*`)
    /// * `sample_cnt` - samples per pixel, or 0 to disable full scene anti-aliasing
    /// * `color_space` - range of colors; may be `None`
    /// * `surface_props` - LCD striping orientation and setting for device independent
    ///                        fonts; may be `None`
    /// Returns: created [`Surface`], or `None`
    #[allow(clippy::missing_safety_doc)]
    #[cfg(feature = "metal")]
    pub unsafe fn from_mtk_view(
        context: &mut gpu::RecordingContext,
        mtk_view: gpu::mtl::Handle,
        origin: gpu::SurfaceOrigin,
        sample_count: impl Into<Option<usize>>,
        color_type: crate::ColorType,
        color_space: impl Into<Option<crate::ColorSpace>>,
        surface_props: Option<&SurfaceProps>,
    ) -> Option<Self> {
        Self::from_ptr(sb::C_SkSurface_MakeFromMTKView(
            context.native_mut(),
            mtk_view,
            origin,
            sample_count.into().unwrap_or(0).try_into().unwrap(),
            color_type.into_native(),
            color_space.into().into_ptr_or_null(),
            surface_props.native_ptr_or_null(),
        ))
    }
}

impl Surface {
    /// Is this surface compatible with the provided characterization?
    ///
    /// This method can be used to determine if an existing [`Surface`] is a viable destination
    /// for an [`DeferredDisplayList`].
    ///
    /// * `characterization` - The characterization for which a compatibility check is desired
    /// Returns: `true` if this surface is compatible with the characterization;
    ///                          `false` otherwise
    pub fn is_compatible(&self, characterization: &SurfaceCharacterization) -> bool {
        unsafe { self.native().isCompatible(characterization.native()) }
    }

    /// Returns [`Surface`] without backing pixels. Drawing to [`Canvas`] returned from [`Surface`]
    /// has no effect. Calling [`Self::image_snapshot()`] on returned [`Surface`] returns `None`.
    ///
    /// * `width` - one or greater
    /// * `height` - one or greater
    /// Returns: [`Surface`] if width and height are positive; otherwise, `None`
    ///
    /// example: <https://fiddle.skia.org/c/@Surface_MakeNull>
    pub fn new_null(size: impl Into<ISize>) -> Option<Self> {
        let size = size.into();
        Self::from_ptr(unsafe { sb::C_SkSurface_MakeNull(size.width, size.height) })
    }

    /// Returns pixel count in each row; may be zero or greater.
    ///
    /// Returns: number of pixel columns
    pub fn width(&self) -> i32 {
        unsafe { sb::C_SkSurface_width(self.native()) }
    }

    /// Returns pixel row count; may be zero or greater.
    ///
    /// Returns: number of pixel rows
    ///
    pub fn height(&self) -> i32 {
        unsafe { sb::C_SkSurface_height(self.native()) }
    }

    /// Returns an [`ImageInfo`] describing the surface.
    pub fn image_info(&mut self) -> ImageInfo {
        let mut info = ImageInfo::default();
        unsafe { sb::C_SkSurface_imageInfo(self.native_mut(), info.native_mut()) };
        info
    }

    /// Returns unique value identifying the content of [`Surface`]. Returned value changes
    /// each time the content changes. Content is changed by drawing, or by calling
    /// [`Self::notify_content_will_change()`].
    ///
    /// Returns: unique content identifier
    ///
    /// example: <https://fiddle.skia.org/c/@Surface_notifyContentWillChange>
    pub fn generation_id(&mut self) -> u32 {
        unsafe { self.native_mut().generationID() }
    }

    /// Notifies that [`Surface`] contents will be changed by code outside of Skia.
    /// Subsequent calls to [`Self::generation_id()`] return a different value.
    ///
    /// example: <https://fiddle.skia.org/c/@Surface_notifyContentWillChange>
    pub fn notify_content_will_change(&mut self, mode: ContentChangeMode) -> &mut Self {
        unsafe { self.native_mut().notifyContentWillChange(mode) }
        self
    }
}

#[cfg(feature = "gpu")]
impl Surface {
    /// Returns the recording context being used by the [`Surface`].
    ///
    /// Returns: the recording context, if available; `None` otherwise
    pub fn recording_context(&mut self) -> Option<gpu::RecordingContext> {
        gpu::RecordingContext::from_unshared_ptr(unsafe { self.native_mut().recordingContext() })
    }

    /// Retrieves the back-end texture. If [`Surface`] has no back-end texture, `None`
    /// is returned.
    ///
    /// The returned [`gpu::BackendTexture`] should be discarded if the [`Surface`] is drawn to or deleted.
    ///
    /// Returns: GPU texture reference; `None` on failure
    pub fn get_backend_texture(
        &mut self,
        handle_access: BackendHandleAccess,
    ) -> Option<gpu::BackendTexture> {
        unsafe {
            let ptr = sb::C_SkSurface_getBackendTexture(self.native_mut(), handle_access);
            gpu::BackendTexture::from_native_if_valid(ptr)
        }
    }

    /// Retrieves the back-end render target. If [`Surface`] has no back-end render target, `None`
    /// is returned.
    ///
    /// The returned [`gpu::BackendRenderTarget`] should be discarded if the [`Surface`] is drawn to
    /// or deleted.
    ///
    /// Returns: GPU render target reference; `None` on failure
    pub fn get_backend_render_target(
        &mut self,
        handle_access: BackendHandleAccess,
    ) -> Option<gpu::BackendRenderTarget> {
        unsafe {
            let mut backend_render_target =
                construct(|rt| sb::C_GrBackendRenderTarget_Construct(rt));
            sb::C_SkSurface_getBackendRenderTarget(
                self.native_mut(),
                handle_access,
                &mut backend_render_target,
            );

            gpu::BackendRenderTarget::from_native_c_if_valid(backend_render_target)
        }
    }

    // TODO: support variant with TextureReleaseProc and ReleaseContext

    /// If the surface was made via [`Self::from_backend_texture`] then it's backing texture may be
    /// substituted with a different texture. The contents of the previous backing texture are
    /// copied into the new texture. [`Canvas`] state is preserved. The original sample count is
    /// used. The [`gpu::BackendFormat`] and dimensions of replacement texture must match that of
    /// the original.
    ///
    /// * `backend_texture` - the new backing texture for the surface
    pub fn replace_backend_texture(
        &mut self,
        backend_texture: &gpu::BackendTexture,
        origin: gpu::SurfaceOrigin,
    ) -> bool {
        self.replace_backend_texture_with_mode(backend_texture, origin, ContentChangeMode::Retain)
    }

    /// If the surface was made via [`Self::from_backend_texture()`] then it's backing texture may be
    /// substituted with a different texture. The contents of the previous backing texture are
    /// copied into the new texture. [`Canvas`] state is preserved. The original sample count is
    /// used. The [`gpu::BackendFormat`] and dimensions of replacement texture must match that of
    /// the original.
    ///
    /// * `backend_texture` - the new backing texture for the surface
    /// * `mode` - Retain or discard current Content
    pub fn replace_backend_texture_with_mode(
        &mut self,
        backend_texture: &gpu::BackendTexture,
        origin: gpu::SurfaceOrigin,
        mode: impl Into<Option<ContentChangeMode>>,
    ) -> bool {
        unsafe {
            self.native_mut().replaceBackendTexture(
                backend_texture.native(),
                origin,
                mode.into().unwrap_or(ContentChangeMode::Retain),
                None,
                ptr::null_mut(),
            )
        }
    }
}

impl Surface {
    /// Returns [`Canvas`] that draws into [`Surface`]. Subsequent calls return the same [`Canvas`].
    /// [`Canvas`] returned is managed and owned by [`Surface`], and is deleted when [`Surface`]
    /// is deleted.
    ///
    /// Returns: drawing [`Canvas`] for [`Surface`]
    ///
    /// example: <https://fiddle.skia.org/c/@Surface_getCanvas>
    pub fn canvas(&mut self) -> &mut Canvas {
        let canvas_ref = unsafe { &mut *self.native_mut().getCanvas() };
        Canvas::borrow_from_native_mut(canvas_ref)
    }

    // TODO: capabilities()

    // TODO: why is self mutable here?

    /// Returns a compatible [`Surface`], or `None`. Returned [`Surface`] contains
    /// the same raster, GPU, or null properties as the original. Returned [`Surface`]
    /// does not share the same pixels.
    ///
    /// Returns `None` if `image_info` width or height are zero, or if `image_info`
    /// is incompatible with [`Surface`].
    ///
    /// * `image_info` - width, height, [`crate::ColorType`], [`crate::AlphaType`], [`crate::ColorSpace`],
    ///                   of [`Surface`]; width and height must be greater than zero
    /// Returns: compatible [`Surface`] or `None`
    ///
    /// example: <https://fiddle.skia.org/c/@Surface_makeSurface>
    pub fn new_surface(&mut self, image_info: &ImageInfo) -> Option<Self> {
        Self::from_ptr(unsafe {
            sb::C_SkSurface_makeSurface(self.native_mut(), image_info.native())
        })
    }

    /// Calls [`Self::new_surface()`] with the same [`ImageInfo`] as this surface, but with the
    /// specified width and height.
    pub fn new_surface_with_dimensions(&mut self, dim: impl Into<ISize>) -> Option<Self> {
        let dim = dim.into();
        Self::from_ptr(unsafe {
            sb::C_SkSurface_makeSurface2(self.native_mut(), dim.width, dim.height)
        })
    }

    /// Returns [`Image`] capturing [`Surface`] contents. Subsequent drawing to [`Surface`] contents
    /// are not captured. [`Image`] allocation is accounted for if [`Surface`] was created with
    /// [`gpu::Budgeted::Yes`].
    ///
    /// Returns: [`Image`] initialized with [`Surface`] contents
    ///
    /// example: <https://fiddle.skia.org/c/@Surface_makeImageSnapshot>
    pub fn image_snapshot(&mut self) -> Image {
        Image::from_ptr(unsafe {
            sb::C_SkSurface_makeImageSnapshot(self.native_mut(), ptr::null())
        })
        .unwrap()
    }

    // TODO: combine this function with image_snapshot and make bounds optional()?

    /// Like the no-parameter version, this returns an image of the current surface contents.
    /// This variant takes a rectangle specifying the subset of the surface that is of interest.
    /// These bounds will be sanitized before being used.
    /// - If bounds extends beyond the surface, it will be trimmed to just the intersection of
    ///   it and the surface.
    /// - If bounds does not intersect the surface, then this returns `None`.
    /// - If bounds == the surface, then this is the same as calling the no-parameter variant.
    ///
    /// example: <https://fiddle.skia.org/c/@Surface_makeImageSnapshot_2>
    pub fn image_snapshot_with_bounds(&mut self, bounds: impl AsRef<IRect>) -> Option<Image> {
        Image::from_ptr(unsafe {
            sb::C_SkSurface_makeImageSnapshot(self.native_mut(), bounds.as_ref().native())
        })
    }

    /// Draws [`Surface`] contents to canvas, with its top-left corner at `(offset.x, offset.y)`.
    ///
    /// If [`Paint`] paint is not `None`, apply [`crate::ColorFilter`], alpha, [`crate::ImageFilter`], and [`crate::BlendMode`].
    ///
    /// * `canvas` - [`Canvas`] drawn into
    /// * `offset.x` - horizontal offset in [`Canvas`]
    /// * `offset.y` - vertical offset in [`Canvas`]
    /// * `sampling` - what technique to use when sampling the surface pixels
    /// * `paint` - [`Paint`] containing [`crate::BlendMode`], [`crate::ColorFilter`], [`crate::ImageFilter`],
    ///                and so on; or `None`
    ///
    /// example: <https://fiddle.skia.org/c/@Surface_draw>
    pub fn draw(
        &mut self,
        canvas: &mut Canvas,
        offset: impl Into<Point>,
        sampling: impl Into<SamplingOptions>,
        paint: Option<&Paint>,
    ) {
        let offset = offset.into();
        let sampling = sampling.into();
        unsafe {
            self.native_mut().draw(
                canvas.native_mut(),
                offset.x,
                offset.y,
                sampling.native(),
                paint.native_ptr_or_null(),
            )
        }
    }

    pub fn peek_pixels(&mut self) -> Option<Borrows<Pixmap>> {
        let mut pm = Pixmap::default();
        unsafe { self.native_mut().peekPixels(pm.native_mut()) }
            .if_true_then_some(move || pm.borrows(self))
    }

    // TODO: why is self mut?

    /// Copies [`crate::Rect`] of pixels to dst.
    ///
    /// Source [`crate::Rect`] corners are (`src.x`, `src.y`) and [`Surface`] `(width(), height())`.
    /// Destination [`crate::Rect`] corners are `(0, 0)` and `(dst.width(), dst.height())`.
    /// Copies each readable pixel intersecting both rectangles, without scaling,
    /// converting to `dst_color_type()` and `dst_alpha_type()` if required.
    ///
    /// Pixels are readable when [`Surface`] is raster, or backed by a GPU.
    ///
    /// The destination pixel storage must be allocated by the caller.
    ///
    /// Pixel values are converted only if [`crate::ColorType`] and [`crate::AlphaType`]
    /// do not match. Only pixels within both source and destination rectangles
    /// are copied. dst contents outside [`crate::Rect`] intersection are unchanged.
    ///
    /// Pass negative values for `src.x` or `src.y` to offset pixels across or down destination.
    ///
    /// Does not copy, and returns `false` if:
    /// - Source and destination rectangles do not intersect.
    /// - [`Pixmap`] pixels could not be allocated.
    /// - `dst.row_bytes()` is too small to contain one row of pixels.
    ///
    /// * `dst` - storage for pixels copied from [`Surface`]
    /// * `src_x` - offset into readable pixels on x-axis; may be negative
    /// * `src_y` - offset into readable pixels on y-axis; may be negative
    /// Returns: `true` if pixels were copied
    ///
    /// example: <https://fiddle.skia.org/c/@Surface_readPixels>    
    pub fn read_pixels_to_pixmap(&mut self, dst: &Pixmap, src: impl Into<IPoint>) -> bool {
        let src = src.into();
        unsafe { self.native_mut().readPixels(dst.native(), src.x, src.y) }
    }

    /// Copies [`crate::Rect`] of pixels from [`Canvas`] into `dst_pixels`.
    ///
    /// Source [`crate::Rect`] corners are (`src.x`, `src.y`) and [`Surface`] (width(), height()).
    /// Destination [`crate::Rect`] corners are (0, 0) and (`dst_info`.width(), `dst_info`.height()).
    /// Copies each readable pixel intersecting both rectangles, without scaling,
    /// converting to `dst_info_color_type()` and `dst_info_alpha_type()` if required.
    ///
    /// Pixels are readable when [`Surface`] is raster, or backed by a GPU.
    ///
    /// The destination pixel storage must be allocated by the caller.
    ///
    /// Pixel values are converted only if [`crate::ColorType`] and [`crate::AlphaType`]
    /// do not match. Only pixels within both source and destination rectangles
    /// are copied. `dst_pixels` contents outside [`crate::Rect`] intersection are unchanged.
    ///
    /// Pass negative values for `src.x` or `src.y` to offset pixels across or down destination.
    ///
    /// Does not copy, and returns `false` if:
    /// - Source and destination rectangles do not intersect.
    /// - [`Surface`] pixels could not be converted to `dst_info.color_type()` or `dst_info.alpha_type()`.
    /// - `dst_row_bytes` is too small to contain one row of pixels.
    ///
    /// * `dst_info` - width, height, [`crate::ColorType`], and [`crate::AlphaType`] of `dst_pixels`
    /// * `dst_pixels` - storage for pixels; `dst_info.height()` times `dst_row_bytes`, or larger
    /// * `dst_row_bytes` - size of one destination row; `dst_info.width()` times pixel size, or larger
    /// * `src.x` - offset into readable pixels on x-axis; may be negative
    /// * `src.y` - offset into readable pixels on y-axis; may be negative
    /// Returns: `true` if pixels were copied
    pub fn read_pixels(
        &mut self,
        dst_info: &ImageInfo,
        dst_pixels: &mut [u8],
        dst_row_bytes: usize,
        src: impl Into<IPoint>,
    ) -> bool {
        if !dst_info.valid_pixels(dst_row_bytes, dst_pixels) {
            return false;
        }
        let src = src.into();
        unsafe {
            self.native_mut().readPixels1(
                dst_info.native(),
                dst_pixels.as_mut_ptr() as _,
                dst_row_bytes,
                src.x,
                src.y,
            )
        }
    }

    // TODO: why is self mut?
    // TODO: why is Bitmap immutable?

    /// Copies [`crate::Rect`] of pixels from [`Surface`] into bitmap.
    ///
    /// Source [`crate::Rect`] corners are (`src.x`, `src.y`) and [`Surface`] (width(), height()).
    /// Destination [`crate::Rect`] corners are `(0, 0)` and `(bitmap.width(), bitmap.height())`.
    /// Copies each readable pixel intersecting both rectangles, without scaling,
    /// converting to `bitmap.color_type()` and `bitmap.alpha_type()` if required.
    ///
    /// Pixels are readable when [`Surface`] is raster, or backed by a GPU.
    ///
    /// The destination pixel storage must be allocated by the caller.
    ///
    /// Pixel values are converted only if [`crate::ColorType`] and [`crate::AlphaType`]
    /// do not match. Only pixels within both source and destination rectangles
    /// are copied. dst contents outside [`crate::Rect`] intersection are unchanged.
    ///
    /// Pass negative values for `src.x` or `src.y` to offset pixels across or down destination.
    ///
    /// Does not copy, and returns `false` if:
    /// - Source and destination rectangles do not intersect.
    /// - [`Surface`] pixels could not be converted to `dst.color_type()` or `dst.alpha_type()`.
    /// - dst pixels could not be allocated.
    /// - `dst.row_bytes()` is too small to contain one row of pixels.
    ///
    /// * `dst` - storage for pixels copied from [`Surface`]
    /// * `src.x` - offset into readable pixels on x-axis; may be negative
    /// * `src.y` - offset into readable pixels on y-axis; may be negative
    /// Returns: `true` if pixels were copied
    ///
    /// example: <https://fiddle.skia.org/c/@Surface_readPixels_3>
    pub fn read_pixels_to_bitmap(&mut self, bitmap: &Bitmap, src: impl Into<IPoint>) -> bool {
        let src = src.into();
        unsafe { self.native_mut().readPixels2(bitmap.native(), src.x, src.y) }
    }

    // TODO: AsyncReadResult, RescaleGamma (m79, m86)
    // TODO: wrap asyncRescaleAndReadPixels (m76, m79, m89)
    // TODO: wrap asyncRescaleAndReadPixelsYUV420 (m77, m79, m89)

    /// Copies [`crate::Rect`] of pixels from the src [`Pixmap`] to the [`Surface`].
    ///
    /// Source [`crate::Rect`] corners are `(0, 0)` and `(src.width(), src.height())`.
    /// Destination [`crate::Rect`] corners are `(`dst.x`, `dst.y`)` and
    /// (`dst.x` + Surface width(), `dst.y` + Surface height()).
    ///
    /// Copies each readable pixel intersecting both rectangles, without scaling,
    /// converting to [`Surface`] `color_type()` and [`Surface`] `alpha_type()` if required.
    ///
    /// * `src` - storage for pixels to copy to [`Surface`]
    /// * `dst.x` - x-axis position relative to [`Surface`] to begin copy; may be negative
    /// * `dst.y` - y-axis position relative to [`Surface`] to begin copy; may be negative
    ///
    /// example: <https://fiddle.skia.org/c/@Surface_writePixels>
    pub fn write_pixels_from_pixmap(&mut self, src: &Pixmap, dst: impl Into<IPoint>) {
        let dst = dst.into();
        unsafe { self.native_mut().writePixels(src.native(), dst.x, dst.y) }
    }

    /// Copies [`crate::Rect`] of pixels from the src [`Bitmap`] to the [`Surface`].
    ///
    /// Source [`crate::Rect`] corners are `(0, 0)` and `(src.width(), src.height())`.
    /// Destination [`crate::Rect`] corners are `(`dst.x`, `dst.y`)` and
    /// `(`dst.x` + Surface width(), `dst.y` + Surface height())`.
    ///
    /// Copies each readable pixel intersecting both rectangles, without scaling,
    /// converting to [`Surface`] `color_type()` and [`Surface`] `alpha_type()` if required.
    ///
    /// * `src` - storage for pixels to copy to [`Surface`]
    /// * `dst.x` - x-axis position relative to [`Surface`] to begin copy; may be negative
    /// * `dst.y` - y-axis position relative to [`Surface`] to begin copy; may be negative
    ///
    /// example: <https://fiddle.skia.org/c/@Surface_writePixels_2>
    pub fn write_pixels_from_bitmap(&mut self, bitmap: &Bitmap, dst: impl Into<IPoint>) {
        let dst = dst.into();
        unsafe {
            self.native_mut()
                .writePixels1(bitmap.native(), dst.x, dst.y)
        }
    }

    /// Returns [`SurfaceProps`] for surface.
    ///
    /// Returns: LCD striping orientation and setting for device independent fonts
    pub fn props(&self) -> &SurfaceProps {
        SurfaceProps::from_native_ref(unsafe { &*sb::C_SkSurface_props(self.native()) })
    }

    /// Call to ensure all reads/writes of the surface have been issued to the underlying 3D API.
    /// Skia will correctly order its own draws and pixel operations. This must to be used to ensure
    /// correct ordering when the surface backing store is accessed outside Skia (e.g. direct use of
    /// the 3D API or a windowing system). [`gpu::DirectContext`] has additional flush and submit methods
    /// that apply to all surfaces and images created from a [`gpu::DirectContext`]. This is equivalent
    /// to calling [`Self::flush()`] with a default [`gpu::FlushInfo`] followed by
    /// [`gpu::DirectContext::submit`].
    pub fn flush_and_submit(&mut self) {
        unsafe {
            self.native_mut().flushAndSubmit(false);
        }
    }

    /// See [`Self::flush_and_submit()`].
    pub fn flush_submit_and_sync_cpu(&mut self) {
        unsafe {
            self.native_mut().flushAndSubmit(true);
        }
    }

    /// If a surface is GPU texture backed, is being drawn with MSAA, and there is a resolve
    /// texture, this call will insert a resolve command into the stream of gpu commands. In order
    /// for the resolve to actually have an effect, the work still needs to be flushed and submitted
    /// to the GPU after recording the resolve command. If a resolve is not supported or the
    /// [`Surface`] has no dirty work to resolve, then this call is a no-op.
    ///
    /// This call is most useful when the [`Surface`] is created by wrapping a single sampled gpu
    /// texture, but asking Skia to render with MSAA. If the client wants to use the wrapped texture
    /// outside of Skia, the only way to trigger a resolve is either to call this command or use
    /// [`Self::flush()`].
    #[cfg(feature = "gpu")]
    pub fn resolve_msaa(&mut self) {
        unsafe { self.native_mut().resolveMSAA() }
    }

    // After deprecated since 0.30.0 (m85), the default flush() behavior changed in m86.
    // For more information, take a look at the documentation in Skia's SkSurface.h

    /// See [`Self::flush_with_mutable_state()`].
    #[cfg(feature = "gpu")]
    pub fn flush(&mut self) {
        let info = gpu::FlushInfo::default();
        self.flush_with_mutable_state(&info, None);
    }

    /// Issues pending [`Surface`] commands to the GPU-backed API objects and resolves any [`Surface`]
    /// MSAA. A call to [`gpu::DirectContext::submit`] is always required to ensure work is actually sent
    /// to the gpu. Some specific API details:
    ///     GL: Commands are actually sent to the driver, but `gl_flush` is never called. Thus some
    ///         sync objects from the flush will not be valid until a submission occurs.
    ///
    ///     Vulkan/Metal/D3D/Dawn: Commands are recorded to the backend APIs corresponding command
    ///         buffer or encoder objects. However, these objects are not sent to the gpu until a
    ///         submission occurs.
    ///
    /// The work that is submitted to the GPU will be dependent on the BackendSurfaceAccess that is
    /// passed in.
    ///
    /// If [`BackendSurfaceAccess::NoAccess`] is passed in all commands will be issued to the GPU.
    ///
    /// If [`BackendSurfaceAccess::Present`] is passed in and the backend API is not Vulkan, it is
    /// treated the same as `k_no_access`. If the backend API is Vulkan, the VkImage that backs the
    /// [`Surface`] will be transferred back to its original queue. If the [`Surface`] was created by
    /// wrapping a VkImage, the queue will be set to the queue which was originally passed in on
    /// the [`gpu::vk::ImageInfo`]. Additionally, if the original queue was not external or foreign the
    /// layout of the VkImage will be set to `VK_IMAGE_LAYOUT_PRESENT_SRC_KHR`.
    ///
    /// The [`gpu::FlushInfo`] describes additional options to flush. Please see documentation at
    /// [`gpu::FlushInfo`] for more info.
    ///
    /// If the return is [`gpu::SemaphoresSubmitted::Yes`], only initialized `BackendSemaphores` will be
    /// submitted to the gpu during the next submit call (it is possible Skia failed to create a
    /// subset of the semaphores). The client should not wait on these semaphores until after submit
    /// has been called, but must keep them alive until then. If a submit flag was passed in with
    /// the flush these valid semaphores can we waited on immediately. If this call returns
    /// [`gpu::SemaphoresSubmitted::No`], the GPU backend will not submit any semaphores to be signaled on
    /// the GPU. Thus the client should not have the GPU wait on any of the semaphores passed in
    /// with the [`gpu::FlushInfo`]. Regardless of whether semaphores were submitted to the GPU or not, the
    /// client is still responsible for deleting any initialized semaphores.
    /// Regardless of semaphore submission the context will still be flushed. It should be
    /// emphasized that a return value of [`gpu::SemaphoresSubmitted::No`] does not mean the flush did not
    /// happen. It simply means there were no semaphores submitted to the GPU. A caller should only
    /// take this as a failure if they passed in semaphores to be submitted.
    ///
    /// Pending surface commands are flushed regardless of the return result.
    ///
    /// * `access` - type of access the call will do on the backend object after flush
    /// * `info` - flush options
    #[cfg(feature = "gpu")]
    pub fn flush_with_access_info(
        &mut self,
        access: BackendSurfaceAccess,
        info: &gpu::FlushInfo,
    ) -> gpu::SemaphoresSubmitted {
        unsafe { self.native_mut().flush(access, info.native()) }
    }

    /// Issues pending [`Surface`] commands to the GPU-backed API objects and resolves any [`Surface`]
    /// MSAA. A call to [`gpu::DirectContext::submit`] is always required to ensure work is actually sent
    /// to the gpu. Some specific API details:
    ///     GL: Commands are actually sent to the driver, but `gl_flush` is never called. Thus some
    ///         sync objects from the flush will not be valid until a submission occurs.
    ///
    ///     Vulkan/Metal/D3D/Dawn: Commands are recorded to the backend APIs corresponding command
    ///         buffer or encoder objects. However, these objects are not sent to the gpu until a
    ///         submission occurs.
    ///
    /// The [`gpu::FlushInfo`] describes additional options to flush. Please see documentation at
    /// [`gpu::FlushInfo`] for more info.
    ///
    /// If a [`gpu::MutableTextureState`] is passed in, at the end of the flush we will transition
    /// the surface to be in the state requested by the skgpu::MutableTextureState. If the surface
    /// (or [`Image`] or `BackendSurface` wrapping the same backend object) is used again after this
    /// flush the state may be changed and no longer match what is requested here. This is often
    /// used if the surface will be used for presenting or external use and the client wants backend
    /// object to be prepped for that use. A `finished_proc` or semaphore on the [`gpu::FlushInfo`] will also
    /// include the work for any requested state change.
    ///
    /// If the backend API is Vulkan, the caller can set the skgpu::MutableTextureState's
    /// VkImageLayout to VK_IMAGE_LAYOUT_UNDEFINED or `queue_family_index` to VK_QUEUE_FAMILY_IGNORED to
    /// tell Skia to not change those respective states.
    ///
    /// If the return is [`gpu::SemaphoresSubmitted::Yes`], only initialized `BackendSemaphores` will be
    /// submitted to the gpu during the next submit call (it is possible Skia failed to create a
    /// subset of the semaphores). The client should not wait on these semaphores until after submit
    /// has been called, but must keep them alive until then. If a submit flag was passed in with
    /// the flush these valid semaphores can we waited on immediately. If this call returns
    /// [`gpu::SemaphoresSubmitted::No`], the GPU backend will not submit any semaphores to be signaled on
    /// the GPU. Thus the client should not have the GPU wait on any of the semaphores passed in
    /// with the [`gpu::FlushInfo`]. Regardless of whether semaphores were submitted to the GPU or not, the
    /// client is still responsible for deleting any initialized semaphores.
    /// Regardless of semaphore submission the context will still be flushed. It should be
    /// emphasized that a return value of [`gpu::SemaphoresSubmitted::No`] does not mean the flush did not
    /// happen. It simply means there were no semaphores submitted to the GPU. A caller should only
    /// take this as a failure if they passed in semaphores to be submitted.
    ///
    /// Pending surface commands are flushed regardless of the return result.
    ///
    /// * `info` - flush options
    /// * `access` - optional state change request after flush
    #[cfg(feature = "gpu")]
    pub fn flush_with_mutable_state<'a>(
        &mut self,
        info: &gpu::FlushInfo,
        new_state: impl Into<Option<&'a gpu::MutableTextureState>>,
    ) -> gpu::SemaphoresSubmitted {
        unsafe {
            self.native_mut()
                .flush1(info.native(), new_state.into().native_ptr_or_null())
        }
    }

    // TODO: wait()

    /// Initializes [`SurfaceCharacterization`] that can be used to perform GPU back-end
    /// processing in a separate thread. Typically this is used to divide drawing
    /// into multiple tiles. [`crate::DeferredDisplayListRecorder`] records the drawing commands
    /// for each tile.
    ///
    /// Return `true` if [`Surface`] supports characterization. raster surface returns `false`.
    ///
    /// * `characterization` - properties for parallel drawing
    /// Returns: `true` if supported
    ///
    /// example: <https://fiddle.skia.org/c/@Surface_characterize>
    pub fn characterize(&self) -> Option<SurfaceCharacterization> {
        let mut sc = SurfaceCharacterization::default();
        unsafe { self.native().characterize(sc.native_mut()) }.if_true_some(sc)
    }

    /// Draws the deferred display list created via a [`crate::DeferredDisplayListRecorder`].
    /// If the deferred display list is not compatible with this [`Surface`], the draw is skipped
    /// and `false` is return.
    ///
    /// The `offset.x` and `offset.y` parameters are experimental and, if not both zero, will cause
    /// the draw to be ignored.
    /// When implemented, if `offset.x` or `offset.y` are non-zero, the DDL will be drawn offset by that
    /// amount into the surface.
    ///
    /// * `deferred_display_list` - drawing commands
    /// * `offset.x` - x-offset at which to draw the DDL
    /// * `offset.y` - y-offset at which to draw the DDL
    /// Returns: `false` if `deferred_display_list` is not compatible
    ///
    /// example: <https://fiddle.skia.org/c/@Surface_draw_2>
    pub fn draw_display_list_with_offset(
        &mut self,
        deferred_display_list: impl Into<DeferredDisplayList>,
        offset: impl Into<IVector>,
    ) -> bool {
        let offset = offset.into();
        unsafe {
            sb::C_SkSurface_draw(
                self.native_mut(),
                deferred_display_list.into().into_ptr() as *const _,
                offset.x,
                offset.y,
            )
        }
    }

    /// See [`Self::draw_display_list_with_offset`].
    pub fn draw_display_list(
        &mut self,
        deferred_display_list: impl Into<DeferredDisplayList>,
    ) -> bool {
        self.draw_display_list_with_offset(deferred_display_list, IVector::default())
    }
}

#[test]
fn create() {
    assert!(Surface::new_raster_n32_premul((0, 0)).is_none());
    let surface = Surface::new_raster_n32_premul((1, 1)).unwrap();
    assert_eq!(1, surface.native().ref_counted_base()._ref_cnt())
}

#[test]
fn test_raster_direct() {
    let image_info = ImageInfo::new(
        (20, 20),
        crate::ColorType::RGBA8888,
        crate::AlphaType::Unpremul,
        None,
    );
    let min_row_bytes = image_info.min_row_bytes();
    let mut pixels = vec![0u8; image_info.compute_byte_size(min_row_bytes)];
    let mut surface = Surface::new_raster_direct(
        &image_info,
        pixels.as_mut_slice(),
        Some(min_row_bytes),
        None,
    )
    .unwrap();
    let paint = Paint::default();
    surface.canvas().draw_circle((10, 10), 10.0, &paint);
}

#[test]
fn test_drawing_owned_as_exclusive_ref_ergonomics() {
    let mut surface = Surface::new_raster_n32_premul((16, 16)).unwrap();

    // option1:
    // - An &mut canvas can be drawn to.
    {
        let mut canvas = Canvas::new(ISize::new(16, 16), None).unwrap();
        surface.draw(&mut canvas, (5.0, 5.0), SamplingOptions::default(), None);
        surface.draw(&mut canvas, (10.0, 10.0), SamplingOptions::default(), None);
    }

    // option2:
    // - A canvas from another surface can be drawn to.
    {
        let mut surface2 = Surface::new_raster_n32_premul((16, 16)).unwrap();
        let canvas = surface2.canvas();
        surface.draw(canvas, (5.0, 5.0), SamplingOptions::default(), None);
        surface.draw(canvas, (10.0, 10.0), SamplingOptions::default(), None);
    }
}
