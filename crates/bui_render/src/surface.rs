use super::*;

/// Routes generated UI to a specific camera.
///
/// Leave the camera unset to use Bevy's primary UI camera. To make a world-space BUI surface,
/// target a camera whose render target is an [`Image`], then display that
/// image on a Bevy 3D mesh. Camera and render-target creation remain under the host application's
/// control.
#[derive(Default, Resource)]
pub struct RenderTargetCamera(pub Option<Entity>);

/// Describes the mapping between a world-space mesh surface and its offscreen BUI camera.
///
/// Attach this component to a mesh whose local plane lies on XY at `z = 0`, centered at the
/// entity origin. The renderer does not create the camera, render-target image, or mesh. Set
/// [`RenderTargetCamera`] to `target_camera`, render it to an image, and use that image as the
/// mesh texture. Register the same texture handle under `texture_asset_id` when it should also
/// appear in a BUI [`Primitive::Image`].
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct UiWorldSurface {
    pub target_camera: Entity,
    pub texture_asset_id: u64,
    /// Full width and height of the plane in local world units.
    pub world_size: Vec2,
    /// Render-target extent in pixels.
    pub resolution: UVec2,
}

impl UiWorldSurface {
    /// Map a point in the surface's local XY plane to top-left-origin UI pixels.
    ///
    /// The left and top edges are included; the right and bottom edges are excluded, matching
    /// the half-open rectangle semantics used by BUI hit testing.
    pub fn local_to_ui(self, local_xy: Vec2) -> Option<[f32; 2]> {
        let [width, height] = [self.world_size.x, self.world_size.y];
        if !local_xy.is_finite()
            || !width.is_finite()
            || !height.is_finite()
            || width <= 0.0
            || height <= 0.0
            || self.resolution.x == 0
            || self.resolution.y == 0
        {
            return None;
        }

        let half_width = width * 0.5;
        let half_height = height * 0.5;
        if local_xy.x < -half_width
            || local_xy.x >= half_width
            || local_xy.y > half_height
            || local_xy.y <= -half_height
        {
            return None;
        }

        Some([
            (local_xy.x + half_width) / width * self.resolution.x as f32,
            (half_height - local_xy.y) / height * self.resolution.y as f32,
        ])
    }
}
