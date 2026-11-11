use super::*;

pub(super) struct LayoutPass {
    pub(super) window: Entity,
    pub(super) dpi_scale: f32,
    pub(super) visiting: Vec<Entity>,
    pub(super) visited: HashSet<Entity>,
    pub(super) issues: Vec<(Entity, UiLayoutIssue)>,
    pub(super) clamped_scrolls: HashSet<Entity>,
}

pub(super) enum AssignedRect {
    Raw(Rect),
    Resolved(Rect),
}

impl LayoutPass {
    pub(super) fn layout_subtree(
        &mut self,
        world: &mut World,
        entity: Entity,
        assigned: AssignedRect,
        inherited_clip: Option<Rect>,
        required: Option<&HashSet<Entity>>,
        measurements: &mut MeasurementMap,
    ) {
        if self.visiting.contains(&entity) {
            self.issues.push((entity, UiLayoutIssue::Cycle));
            return;
        }
        if world.get::<UiNode>(entity).is_none() {
            self.issues.push((entity, UiLayoutIssue::MissingNode));
            return;
        }

        self.visiting.push(entity);
        self.visited.insert(entity);
        let style = world
            .get::<UiLayoutNode>(entity)
            .copied()
            .unwrap_or_default()
            .sanitized();
        let rect = match assigned {
            AssignedRect::Raw(assigned) => {
                apply_margins_and_constraints(assigned, style.margin, style)
            }
            AssignedRect::Resolved(rect) => rect,
        };
        let scrollable = world.get::<UiScroll>(entity).is_some();
        let own_clip = if style.clip_contents || scrollable {
            Some(inherited_clip.map_or(rect, |parent| {
                parent.intersection(rect).unwrap_or(Rect::ZERO)
            }))
        } else {
            inherited_clip
        };
        let z = style
            .z_index
            .saturating_add(world.get::<UiZIndex>(entity).map_or(0, |z| z.0));
        let mut children = world
            .get::<bbui_core::UiChildren>(entity)
            .map(|children| children.0.clone())
            .unwrap_or_default();
        children.retain(|child| {
            world
                .get::<UiVisible>(*child)
                .is_none_or(|visible| visible.0)
        });
        let content_size = measure_content_with_available(
            world,
            entity,
            style,
            &children,
            [rect.width, rect.height],
            measurements,
        );
        let scroll_offset = if let Some(mut scroll) = world.get_mut::<UiScroll>(entity) {
            if scroll.clamp_to(content_size, [rect.width, rect.height]) {
                self.clamped_scrolls.insert(entity);
            }
            scroll.offset
        } else {
            [0.0, 0.0]
        };
        let result = UiLayoutResult {
            window: self.window,
            rect,
            clip: inherited_clip,
            z,
            content_size,
            dpi_scale: self.dpi_scale,
        };
        let hit_box = UiHitBox {
            window: Some(self.window),
            rect,
            clip: inherited_clip,
            z_index: z,
        };
        if world.get::<UiLayoutResult>(entity).copied() != Some(result) {
            world.entity_mut(entity).insert(result);
        }
        if world.get::<UiHitBox>(entity).copied() != Some(hit_box) {
            world.entity_mut(entity).insert(hit_box);
        }

        if !children.is_empty() && style.mode != UiLayoutMode::Leaf {
            let inner = rect.inset(
                style.padding.left,
                style.padding.top,
                style.padding.right,
                style.padding.bottom,
            );
            let child_rects = distribute_children(world, &children, style, inner, measurements);
            for (child, child_rect) in children.into_iter().zip(child_rects) {
                let child_rect = Rect::new(
                    child_rect.x - scroll_offset[0],
                    child_rect.y - scroll_offset[1],
                    child_rect.width,
                    child_rect.height,
                );
                if required.is_none_or(|required| required.contains(&child))
                    || layout_context_changed(
                        world,
                        child,
                        child_rect,
                        own_clip,
                        self.window,
                        self.dpi_scale,
                    )
                {
                    self.layout_subtree(
                        world,
                        child,
                        AssignedRect::Raw(child_rect),
                        own_clip,
                        required,
                        measurements,
                    );
                }
            }
        }
        self.visiting.pop();
    }
}

pub(super) fn layout_context_changed(
    world: &World,
    entity: Entity,
    assigned: Rect,
    inherited_clip: Option<Rect>,
    window: Entity,
    dpi_scale: f32,
) -> bool {
    let Some(previous) = world.get::<UiLayoutResult>(entity) else {
        return true;
    };
    let style = world
        .get::<UiLayoutNode>(entity)
        .copied()
        .unwrap_or_default()
        .sanitized();
    let rect = apply_margins_and_constraints(assigned, style.margin, style);
    previous.rect != rect
        || previous.clip != inherited_clip
        || previous.window != window
        || previous.dpi_scale != dpi_scale
}
