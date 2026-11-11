use super::*;

pub(super) fn capture_base_layout_node(world: &mut World, entity: Entity) -> UiLayoutNode {
    if let Some(base) = world.get::<UiBaseLayoutNode>(entity) {
        return base.0;
    }
    let base = world
        .get::<UiLayoutNode>(entity)
        .copied()
        .unwrap_or_default();
    world.entity_mut(entity).insert(UiBaseLayoutNode(base));
    base
}

pub(super) fn add_base_layout_values(style: &mut UiComputedStyle, layout: UiLayoutNode) {
    use UiStyleProperty::*;
    let properties = &mut style.properties;
    properties
        .entry(Width)
        .or_insert(UiStyleValue::Length(layout.size.width));
    properties
        .entry(Height)
        .or_insert(UiStyleValue::Length(layout.size.height));
    for (property, value) in [
        (MinWidth, layout.min_size[0]),
        (MinHeight, layout.min_size[1]),
        (MaxWidth, layout.max_size[0]),
        (MaxHeight, layout.max_size[1]),
    ] {
        if let Some(value) = value {
            properties
                .entry(property)
                .or_insert(UiStyleValue::Scalar(value));
        }
    }
    properties
        .entry(Padding)
        .or_insert(UiStyleValue::Edges(layout.padding));
    properties
        .entry(Margin)
        .or_insert(UiStyleValue::Edges(layout.margin));
    properties
        .entry(Gap)
        .or_insert(UiStyleValue::Scalar(layout.gap));
    properties
        .entry(LayoutMode)
        .or_insert(UiStyleValue::LayoutMode(layout.mode));
    properties
        .entry(AlignItems)
        .or_insert(UiStyleValue::Align(layout.align_items));
    properties
        .entry(JustifyContent)
        .or_insert(UiStyleValue::Justify(layout.justify_content));
    properties
        .entry(ClipContents)
        .or_insert(UiStyleValue::Bool(layout.clip_contents));
    properties
        .entry(ZIndex)
        .or_insert(UiStyleValue::Integer(layout.z_index));
}

pub(super) fn apply_layout_style(world: &mut World, entity: Entity, computed: &UiComputedStyle) {
    let base = capture_base_layout_node(world, entity);
    let mut layout = base;
    apply_layout_property(&mut layout, computed, UiStyleProperty::Width);
    apply_layout_property(&mut layout, computed, UiStyleProperty::Height);
    apply_layout_property(&mut layout, computed, UiStyleProperty::MinWidth);
    apply_layout_property(&mut layout, computed, UiStyleProperty::MinHeight);
    apply_layout_property(&mut layout, computed, UiStyleProperty::MaxWidth);
    apply_layout_property(&mut layout, computed, UiStyleProperty::MaxHeight);
    apply_layout_property(&mut layout, computed, UiStyleProperty::Padding);
    apply_layout_property(&mut layout, computed, UiStyleProperty::Margin);
    apply_layout_property(&mut layout, computed, UiStyleProperty::Gap);
    apply_layout_property(&mut layout, computed, UiStyleProperty::LayoutMode);
    apply_layout_property(&mut layout, computed, UiStyleProperty::AlignItems);
    apply_layout_property(&mut layout, computed, UiStyleProperty::JustifyContent);
    apply_layout_property(&mut layout, computed, UiStyleProperty::ClipContents);
    apply_layout_property(&mut layout, computed, UiStyleProperty::ZIndex);
    if world.get::<UiLayoutNode>(entity).copied() != Some(layout) {
        world.entity_mut(entity).insert(layout);
    }
}

pub(super) fn apply_box_shadow_style(
    world: &mut World,
    entity: Entity,
    computed: &UiComputedStyle,
    theme: &Theme,
) {
    if let Some(shadow) = computed.box_shadow() {
        if world.get::<UiBaseBoxShadow>(entity).is_none() {
            let base = world.get::<BoxShadow>(entity).cloned();
            world.entity_mut(entity).insert(UiBaseBoxShadow(base));
        }
        let native = shadow.to_native(theme);
        if world.get::<BoxShadow>(entity) != Some(&native) {
            world.entity_mut(entity).insert(native);
        }
    } else if let Some(base) = world.get::<UiBaseBoxShadow>(entity).cloned() {
        if let Some(base) = base.0 {
            world.entity_mut(entity).insert(base);
        } else {
            world.entity_mut(entity).remove::<BoxShadow>();
        }
        world.entity_mut(entity).remove::<UiBaseBoxShadow>();
    }
}

pub(super) fn apply_layout_property(
    layout: &mut UiLayoutNode,
    style: &UiComputedStyle,
    property: UiStyleProperty,
) {
    use UiStyleProperty::*;
    match (property, style.get(property)) {
        (Width, Some(UiStyleValue::Length(value))) => layout.size.width = value,
        (Height, Some(UiStyleValue::Length(value))) => layout.size.height = value,
        (MinWidth, Some(UiStyleValue::Scalar(value))) => {
            layout.min_size[0] = Some(nonnegative(value))
        }
        (MinHeight, Some(UiStyleValue::Scalar(value))) => {
            layout.min_size[1] = Some(nonnegative(value))
        }
        (MaxWidth, Some(UiStyleValue::Scalar(value))) => {
            layout.max_size[0] = Some(nonnegative(value))
        }
        (MaxHeight, Some(UiStyleValue::Scalar(value))) => {
            layout.max_size[1] = Some(nonnegative(value))
        }
        (Padding, Some(UiStyleValue::Edges(value))) => layout.padding = value,
        (Margin, Some(UiStyleValue::Edges(value))) => layout.margin = value,
        (Gap, Some(UiStyleValue::Scalar(value))) => layout.gap = nonnegative(value),
        (LayoutMode, Some(UiStyleValue::LayoutMode(value))) => layout.mode = value,
        (AlignItems, Some(UiStyleValue::Align(value))) => layout.align_items = value,
        (JustifyContent, Some(UiStyleValue::Justify(value))) => layout.justify_content = value,
        (ClipContents, Some(UiStyleValue::Bool(value))) => layout.clip_contents = value,
        (ZIndex, Some(UiStyleValue::Integer(value))) => layout.z_index = value,
        _ => {}
    }
}

pub(super) fn hierarchy_depth(world: &World, entity: Entity) -> usize {
    let mut depth = 0;
    let mut current = Some(entity);
    let mut seen = Vec::new();
    while let Some(at) = current {
        if seen.contains(&at) {
            break;
        }
        seen.push(at);
        let Some(parent) = world.get::<UiParent>(at) else {
            break;
        };
        current = Some(parent.0);
        depth += 1;
    }
    depth
}

pub(super) fn hierarchy_order(world: &World, entity: Entity) -> u64 {
    let mut path = Vec::new();
    let mut current = Some(entity);
    while let Some(at) = current {
        path.push(at);
        current = world.get::<UiParent>(at).map(|parent| parent.0);
    }
    path.reverse();
    path.into_iter().fold(0u64, |order, item| {
        order.wrapping_mul(65_537).wrapping_add(item.to_bits())
    })
}
