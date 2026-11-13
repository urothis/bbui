use super::*;
use quote::ToTokens;

#[test]
fn nested_declarations_expand_to_a_widget_tree_constructor() {
    let input: proc_macro2::TokenStream = quote! {
        column {
            label("Transform"),
            checkbox("Enabled", true),
            panel("Advanced", false) {
                number("Scale", 1.0, 0.0, 10.0, 0.1),
                text_field("Name", "Cube"),
            },
        }
    };
    let declaration = syn::parse2::<WidgetDecl>(input).unwrap();
    let built = declaration.build();
    let expanded = quote!(::bbui::widgets::WidgetTree::new(#built));
    let parsed: syn::Expr = syn::parse2(expanded.clone()).unwrap();
    assert!(parsed.to_token_stream().to_string().contains("WidgetTree"));
}

#[test]
fn dropdown_requires_literal_options_and_accepts_selected_index_expression() {
    let input: proc_macro2::TokenStream = quote! { dropdown("Mode", ["Object", "Edit"], Some(1)) };
    assert!(syn::parse2::<WidgetDecl>(input).is_ok());
}

#[test]
fn radio_and_icon_controls_accept_model_and_accessible_name() {
    let radio = syn::parse2::<WidgetDecl>(quote! {
        radio_group("Mode", group.clone())
    })
    .unwrap();
    assert!(
        radio
            .build()
            .to_string()
            .contains("WidgetNode :: radio_group")
    );
    let icon = syn::parse2::<WidgetDecl>(quote! {
        icon_button("+", "Add object")
    })
    .unwrap();
    assert!(
        icon.build()
            .to_string()
            .contains("WidgetNode :: icon_button")
    );
    let icon_asset = syn::parse2::<WidgetDecl>(quote! {
        icon_button_asset("icons/add.svg", "Add object")
    })
    .unwrap();
    assert!(icon_asset.build().to_string().contains("icon_button_asset"));
}

#[test]
fn slider_accepts_a_reusable_numeric_slider_model() {
    let declaration = syn::parse2::<WidgetDecl>(quote! {
        slider("Opacity", slider_model.clone())
    })
    .unwrap();
    assert!(
        declaration
            .build()
            .to_string()
            .contains("WidgetNode :: slider")
    );
}

#[test]
fn slider_range_forms_accept_bounds_and_integer_steps() {
    for (declaration, expected) in [
        (
            syn::parse2::<WidgetDecl>(quote! {
                slider_range("Opacity", 0.5, 0.0, 1.0, 0.01)
            })
            .unwrap(),
            "WidgetNode :: slider_range",
        ),
        (
            syn::parse2::<WidgetDecl>(quote! {
                integer_slider_range("Count", 4, 0, 12, 1)
            })
            .unwrap(),
            "WidgetNode :: integer_slider_range",
        ),
    ] {
        assert!(declaration.build().to_string().contains(expected));
    }
}

#[test]
fn layout_containers_and_property_rows_expand_to_public_widget_nodes() {
    let declaration = syn::parse2::<WidgetDecl>(quote! {
        scroll {
            boxed {
                property_row("Scale") { slider_range("Value", 0.5, 0.0, 1.0, 0.1) },
                grid(2) { button("A"), button("B") },
                responsive_grid(96, 4) { button("C") },
                flex_row(true) { button("D") },
                flex_column(false) { button("E") },
                overlay { label("Front"), label("Back") },
                inline { label("One"), label("Two") },
            }
        }
    })
    .unwrap();
    let expanded = declaration.build().to_string();
    for constructor in [
        "WidgetNode :: scroll",
        "WidgetNode :: boxed",
        "WidgetNode :: property_row",
        "WidgetNode :: grid",
        "WidgetNode :: responsive_grid",
        "WidgetNode :: flex",
        "WidgetNode :: overlay",
        "WidgetNode :: inline",
    ] {
        assert!(expanded.contains(constructor), "missing {constructor}");
    }
    assert!(
        syn::parse2::<WidgetDecl>(quote! {
            property_row("Broken") { button("A"), button("B") }
        })
        .is_err()
    );
    assert!(
        syn::parse2::<WidgetDecl>(quote! {
            flex_row(wrap_from_state) { button("Dynamic") }
        })
        .is_ok()
    );
}

#[test]
fn menus_and_menu_bars_accept_runtime_entry_expressions() {
    for (source, expected) in [
        (
            quote! { menu("File", entries.clone()) },
            "WidgetNode :: menu",
        ),
        (
            quote! { pie_menu("Radial", entries.clone()) },
            "WidgetNode :: pie_menu",
        ),
        (
            quote! { popover("Inspector", entries.clone()) },
            "WidgetNode :: popover",
        ),
        (
            quote! { context_menu("Object", entries.clone()) },
            "WidgetNode :: context_menu",
        ),
    ] {
        let declaration = syn::parse2::<WidgetDecl>(source).unwrap();
        assert!(declaration.build().to_string().contains(expected));
    }

    let declaration = syn::parse2::<WidgetDecl>(quote! {
        menu_bar {
            menu("File", entries.clone()),
            menu("Edit", entries.clone()),
        }
    })
    .unwrap();
    assert!(
        declaration
            .build()
            .to_string()
            .contains("WidgetNode :: menu_bar")
    );
}

#[test]
fn tooltip_wraps_exactly_one_child_widget() {
    let declaration = syn::parse2::<WidgetDecl>(quote! {
        tooltip("Apply changes") {
            button("Apply"),
        }
    })
    .unwrap();
    assert!(
        declaration
            .build()
            .to_string()
            .contains("WidgetNode :: tooltip")
    );
    assert!(
        syn::parse2::<WidgetDecl>(quote! {
            tooltip("bad") { button("One"), button("Two") }
        })
        .is_err()
    );
}

#[test]
fn file_browser_accepts_a_runtime_path_expression() {
    let input: proc_macro2::TokenStream = quote! { file_browser(asset_root.clone()) };
    let declaration = syn::parse2::<WidgetDecl>(input).unwrap();
    assert!(declaration.build().to_string().contains("file_browser"));
}

#[test]
fn color_ramp_accepts_model_or_default_constructor() {
    let explicit: proc_macro2::TokenStream = quote! {
        color_ramp("Gradient", ramp.clone())
    };
    let default: proc_macro2::TokenStream = quote! {
        color_ramp_default("Gradient")
    };
    assert!(
        syn::parse2::<WidgetDecl>(explicit)
            .unwrap()
            .build()
            .to_string()
            .contains("color_ramp")
    );
    assert!(
        syn::parse2::<WidgetDecl>(default)
            .unwrap()
            .build()
            .to_string()
            .contains("color_ramp_default")
    );
}

#[test]
fn hsv_color_accepts_srgb_color_or_default_constructor() {
    let explicit: proc_macro2::TokenStream = quote! {
        hsv_color("Picker", color)
    };
    let default: proc_macro2::TokenStream = quote! {
        hsv_color_default("Picker")
    };
    assert!(
        syn::parse2::<WidgetDecl>(explicit)
            .unwrap()
            .build()
            .to_string()
            .contains("hsv_color")
    );
    assert!(
        syn::parse2::<WidgetDecl>(default)
            .unwrap()
            .build()
            .to_string()
            .contains("hsv_color_default")
    );
}

#[test]
fn curve_editor_accepts_model_or_default_constructor() {
    let explicit: proc_macro2::TokenStream = quote! {
        curve_editor("Ease", curve.clone())
    };
    let default: proc_macro2::TokenStream = quote! {
        curve_editor_default("Ease")
    };
    assert!(
        syn::parse2::<WidgetDecl>(explicit)
            .unwrap()
            .build()
            .to_string()
            .contains("curve_editor")
    );
    assert!(
        syn::parse2::<WidgetDecl>(default)
            .unwrap()
            .build()
            .to_string()
            .contains("curve_editor_default")
    );
}

#[test]
fn collection_widgets_accept_models_or_default_constructors() {
    for (source, expected) in [
        (quote! { list("Objects", model.clone()) }, "list"),
        (quote! { list_default("Objects") }, "list_default"),
        (quote! { tree("Hierarchy", model.clone()) }, "tree"),
        (quote! { tree_default("Hierarchy") }, "tree_default"),
        (
            quote! { preview_grid("Assets", model.clone()) },
            "preview_grid",
        ),
        (
            quote! { preview_grid_default("Assets") },
            "preview_grid_default",
        ),
    ] {
        assert!(
            syn::parse2::<WidgetDecl>(source)
                .unwrap()
                .build()
                .to_string()
                .contains(expected)
        );
    }
}

#[test]
fn data_table_accepts_model_or_default_constructor() {
    for (source, expected) in [
        (
            quote! { data_table("Inventory", model.clone()) },
            "data_table",
        ),
        (
            quote! { data_table_default("Inventory") },
            "data_table_default",
        ),
    ] {
        assert!(
            syn::parse2::<WidgetDecl>(source)
                .unwrap()
                .build()
                .to_string()
                .contains(expected)
        );
    }
}

#[test]
fn node_graph_accepts_model_or_default_constructor() {
    for (source, expected) in [
        (quote! { node_graph("Shader", graph.clone()) }, "node_graph"),
        (
            quote! { node_graph_default("Shader") },
            "node_graph_default",
        ),
    ] {
        assert!(
            syn::parse2::<WidgetDecl>(source)
                .unwrap()
                .build()
                .to_string()
                .contains(expected)
        );
    }
}

#[test]
fn canvas_accepts_model_or_default_constructor() {
    for (source, expected) in [
        (quote! { canvas("Scene", model.clone()) }, "canvas"),
        (quote! { canvas_default("Scene") }, "canvas_default"),
    ] {
        assert!(
            syn::parse2::<WidgetDecl>(source)
                .unwrap()
                .build()
                .to_string()
                .contains(expected)
        );
    }
}
