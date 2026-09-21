use super::*;
use crate::als_hud::{AlsHudRoot, COLOR_TEXT, viewport_ui_scale};

fn viewport(elapsed: f32) -> (u32, UVec2, f32) {
    if elapsed < 1.3 {
        (0, UVec2::new(1280, 720), 1.0)
    } else if elapsed < 3.3 {
        (1, UVec2::new(960, 540), 1.0)
    } else {
        (2, UVec2::new(960, 540), 2.0)
    }
}

pub(super) fn install(app: &mut App) {
    app.add_systems(PreUpdate, resize.after(drive_manual_integration));
    app.add_systems(
        PostUpdate,
        observe
            .after(bevy::ui::UiSystems::PostLayout)
            .after(crate::show_debug_hud::AlsShowDebugHudSet::Sync)
            .before(observe_manual_integration),
    );
}

fn resize(
    state: Res<ManualIntegrationState>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
) {
    if state.scenario != IntegrationScenario::Interface
        || state.phase != IntegrationPhase::Interface
    {
        return;
    }
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    let (_, size, dpi) = viewport(state.phase_elapsed());
    if window.resolution.scale_factor_override() != Some(dpi) {
        window.resolution.set_scale_factor_override(Some(dpi));
    }
    if window.physical_size() != size {
        window.resolution.set_physical_resolution(size.x, size.y);
    }
}

fn observe(
    state: Res<ManualIntegrationState>,
    mut report: ResMut<ManualIntegrationGateReport>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    nodes: Query<(
        &Name,
        &ComputedNode,
        &bevy::ui::UiGlobalTransform,
        &bevy::ui::ComputedUiRenderTargetInfo,
    )>,
    roots: Query<Entity, With<AlsHudRoot>>,
    controls: Query<(
        &ComputedNode,
        &bevy::ui::UiGlobalTransform,
        Option<&Children>,
        Option<&Text>,
        Option<&TextColor>,
    )>,
) {
    if state.scenario != IntegrationScenario::Interface
        || state.phase != IntegrationPhase::Interface
        || !state.interface_debug_armed
    {
        return;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    let (stage, size, dpi) = viewport(state.phase_elapsed());
    let umg_scale = viewport_ui_scale(size);
    if window.physical_size() != size
        || (window.scale_factor() - dpi).abs() > 1e-5
        || nodes.iter().any(|(_, _, _, target)| {
            target.physical_size() != size || (target.scale_factor() - umg_scale).abs() > 1e-5
        })
    {
        report.direction_trace.push(serde_json::json!({"hud_layout_pending":stage,"hud_viewport":window.physical_size().to_array(),
            "hud_dpi":window.scale_factor(),"hud_requested_viewport":size.to_array(),"hud_requested_dpi":dpi}));
        return;
    }
    let scale = (size.x as f32 / 1280.0).min(size.y as f32 / 720.0);
    let mut failures = Vec::new();
    for (name, position) in [
        (
            "DebugHeader:Als.CameraCurves (Shift + 6)",
            Vec2::new(5.0, 5.0),
        ),
        (
            "DebugHeader:Als.CameraShapes (Shift + 7)",
            Vec2::new(205.0, 5.0),
        ),
        ("Camera Offset X", Vec2::new(150.0, 24.0)),
        (
            "DebugHeader:Debug mode is enabled! Press (Shift + 0) to disable.",
            Vec2::new(5.0, 192.0),
        ),
        ("DebugHeader:Als.State (Shift + 2)", Vec2::new(205.0, 219.0)),
        ("Allow Transitions", Vec2::new(150.0, 238.0)),
        ("View Block", Vec2::new(150.0, 742.0)),
    ] {
        let actual = nodes
            .iter()
            .find(|(key, _, _, _)| key.as_str() == name)
            .filter(|(_, node, _, _)| node.size().min_element() > 0.0)
            .map(|(_, node, transform, _)| transform.affine().translation - node.size() * 0.5);
        if actual.is_none_or(|actual| !actual.abs_diff_eq(position * scale, 0.001)) {
            failures.push(serde_json::json!({"field":name,"expected":(position * scale).to_array(),"actual":actual.map(|point| point.to_array())}));
        }
    }
    if let Ok(root) = roots.single() {
        let (root_node, root_transform, sections, _, _) = controls.get(root).unwrap();
        let origin = root_transform.affine().translation - root_node.size() * 0.5;
        let expected = Vec2::new(size.x as f32 - 365.0 * umg_scale, 25.0 * umg_scale);
        if !origin.abs_diff_eq(expected, 1.0)
            || (root_node.size().x - 340.0 * umg_scale).abs() > 1.0
        {
            failures.push(serde_json::json!({"field":"ControlPanel","expected":expected.to_array(),"actual":origin.to_array(),"width":root_node.size().x}));
        }
        for section in sections.into_iter().flatten() {
            let (section_node, _, rows, _, _) = controls.get(*section).unwrap();
            if section_node.size().y == 0.0 {
                continue;
            }
            for (row_index, row) in rows.into_iter().flatten().enumerate() {
                let (node, transform, cells, _, _) = controls.get(*row).unwrap();
                let position = transform.affine().translation - node.size() * 0.5;
                if (position.x - origin.x).abs() > 1.0
                    || (node.size().x - root_node.size().x).abs() > 1.0
                {
                    failures.push(serde_json::json!({"field":"ControlRow","row":row_index,"position":position.to_array(),"width":node.size().x}));
                }
                if row_index == 0 {
                    continue;
                }
                let mut end = origin.x;
                for (index, cell) in cells.into_iter().flatten().enumerate() {
                    let (cell_node, transform, _, _, _) = controls.get(*cell).unwrap();
                    let position = transform.affine().translation - cell_node.size() * 0.5;
                    let gap = if index == 0 { 0.0 } else { 4.0 * umg_scale };
                    if (position.x - end - gap).abs() > 1.0 {
                        failures.push(serde_json::json!({"field":"ControlCell","row":row_index,"column":index,"gap":position.x-end,"expected":gap}));
                    }
                    end = position.x + cell_node.size().x;
                }
                if (end - origin.x - root_node.size().x).abs() > 1.0 {
                    failures.push(
                        serde_json::json!({"field":"ControlRowEnd","row":row_index,"end":end}),
                    );
                }
            }
        }
        if !controls.iter().any(|(_, _, _, text, color)| {
            text.is_some_and(|text| text.0 == "Shift + 0")
                && color.is_some_and(|color| color.0 == COLOR_TEXT)
        }) {
            failures.push(serde_json::json!({"field":"MasterKeyColor"}));
        }
    } else {
        failures.push(serde_json::json!({"field":"ControlPanelMissing"}));
    }
    report.direction_trace.push(
        serde_json::json!({"hud_layout_stage":stage,"hud_canvas_valid":failures.is_empty(),
        "hud_viewport":size.to_array(),"hud_dpi":dpi,"hud_layout_failures":failures}),
    );
}

pub(super) fn passed(trace: &[serde_json::Value]) -> bool {
    (0..3).all(|stage| {
        trace.iter().any(|entry| {
            entry["hud_layout_stage"].as_u64() == Some(stage) && entry["hud_canvas_valid"] == true
        })
    }) && trace
        .iter()
        .filter_map(|entry| {
            entry
                .get("hud_canvas_valid")
                .and_then(serde_json::Value::as_bool)
        })
        .all(|valid| valid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_gate_requires_every_size_and_rejects_late_layout_failures() {
        let mut trace: Vec<_> = (0..3)
            .map(|stage| serde_json::json!({"hud_layout_stage":stage,"hud_canvas_valid":true}))
            .collect();
        assert!(passed(&trace));
        assert!(!passed(&trace[..2]));
        trace.push(serde_json::json!({"hud_layout_stage":2,"hud_canvas_valid":false}));
        assert!(!passed(&trace));
        assert_eq!(viewport(0.0).0, 0);
        assert_eq!(viewport(2.0).0, 1);
        assert_eq!(viewport(4.0).0, 2);
    }
}
