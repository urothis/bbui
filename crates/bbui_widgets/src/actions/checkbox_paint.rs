use super::*;

#[derive(Debug, Default)]
pub(super) struct PaintTransaction {
    pub(super) target: bool,
    pub(super) visited: HashSet<Entity>,
    pub(super) changes: Vec<CheckboxPaintChange>,
}

impl PaintTransaction {
    pub(super) fn begin(entity: Entity, checkbox: &mut Checkbox) -> Option<Self> {
        if !checkbox.enabled || checkbox.read_only {
            return None;
        }
        let before = checkbox.value;
        let target = before != CheckboxValue::On;
        let after = if target {
            CheckboxValue::On
        } else {
            CheckboxValue::Off
        };
        if before != after {
            checkbox.value = after;
        }
        Some(Self {
            target,
            visited: [entity].into(),
            changes: (before != after)
                .then_some(CheckboxPaintChange {
                    entity,
                    before,
                    after,
                })
                .into_iter()
                .collect(),
        })
    }
}
