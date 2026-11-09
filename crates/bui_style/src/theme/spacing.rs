/// Spacing for common control and editor families. These values are logical
/// pixels and are intentionally separate from colors so a theme can keep
/// menu, list, table, and editor geometry coherent across a live switch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeControlSpacing {
    pub padding_x: f32,
    pub padding_y: f32,
    pub gap: f32,
    pub checkbox_label_gap: f32,
    pub error_outline_width: f32,
    pub slider_height: f32,
    /// Logical height of the painted slider track, independent of its hit area.
    pub slider_track_height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemePanelSpacing {
    pub header_height: f32,
    pub content_padding: f32,
    pub section_gap: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeMenuSpacing {
    pub item_height: f32,
    pub padding_x: f32,
    pub padding_y: f32,
    pub section_gap: f32,
    pub popup_padding: f32,
    pub pie_inner_inset: f32,
    pub pie_inner_radius: f32,
    pub pie_outer_radius: f32,
    pub pie_popup_diameter: f32,
    pub pie_outer_item_radius: f32,
    pub pie_nested_item_radius: f32,
    pub pie_item_max_width: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeListSpacing {
    pub row_height: f32,
    pub padding_x: f32,
    pub padding_y: f32,
    pub indent: f32,
    pub section_gap: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeTableSpacing {
    pub header_height: f32,
    pub row_height: f32,
    pub cell_padding_x: f32,
    pub cell_padding_y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeEditorSpacing {
    pub padding_x: f32,
    pub padding_y: f32,
    pub gutter_width: f32,
    pub toolbar_gap: f32,
    pub handle_radius: f32,
    pub grid_spacing: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeCurveSpacing {
    pub selected_status_width: f32,
    pub interpolation_status_width: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeHsvSpacing {
    pub wheel_center_x: f32,
    pub wheel_center_y: f32,
    pub wheel_outer_radius: f32,
    pub wheel_inner_radius: f32,
    pub preview_right_inset: f32,
    pub preview_top_inset: f32,
    pub preview_width: f32,
    pub preview_height: f32,
    pub controls_x: f32,
    pub value_label_y: f32,
    pub value_track_y: f32,
    pub value_track_width_inset: f32,
    pub value_track_height: f32,
    pub alpha_label_y: f32,
    pub alpha_track_y: f32,
    pub alpha_track_width_inset: f32,
    pub alpha_field_y: f32,
    pub alpha_field_right_inset: f32,
    pub alpha_field_width: f32,
    pub field_cell_width: f32,
    pub field_gap: f32,
    pub field_first_row_y: f32,
    pub field_second_row_y: f32,
    pub field_hex_y: f32,
    pub field_hex_width_inset: f32,
    pub field_height: f32,
    pub field_text_inset_x: f32,
    pub field_text_inset_y: f32,
    pub help_y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeColorRampSpacing {
    pub title_x: f32,
    pub title_y: f32,
    pub button_width: f32,
    pub button_step: f32,
    pub button_right_inset: f32,
    pub button_top_inset: f32,
    pub button_height: f32,
    pub header_hit_height: f32,
    pub button_label_x: f32,
    pub button_label_y: f32,
    pub track_x: f32,
    pub track_y: f32,
    pub track_width_inset: f32,
    pub track_height: f32,
    pub marker_line_extension: f32,
    pub marker_hit_extension: f32,
    pub track_hit_top_inset: f32,
    pub marker_handle_radius: f32,
    pub marker_handle_top_inset: f32,
    pub details_x: f32,
    pub details_y: f32,
    pub channel_x: f32,
    pub channel_y: f32,
    pub channel_step: f32,
    pub channel_width_inset: f32,
    pub channel_height: f32,
    pub channel_selection_hit_slop: f32,
    pub channel_click_hit_slop: f32,
    pub channel_label_hit_slop: f32,
    pub channel_label_x: f32,
    pub channel_label_y: f32,
    pub channel_value_right_inset: f32,
    pub editing_right_inset: f32,
    pub editing_bottom_inset: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeTooltipSpacing {
    pub padding_x: f32,
    pub padding_y: f32,
    pub maximum_width: f32,
    pub viewport_margin: f32,
    pub anchor_gap: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeBoxSpacing {
    pub padding: f32,
    pub gap: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemePropertyRowSpacing {
    pub label_width: f32,
    pub gap: f32,
    pub row_height: f32,
}

/// Logical geometry used by node graph editors before applying the graph zoom.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeNodeGraphSpacing {
    pub header_height: f32,
    pub header_hint_width: f32,
    pub body_padding: f32,
    pub socket_spacing: f32,
    pub socket_radius: f32,
    pub socket_hit_slop: f32,
    pub connection_tangent_min: f32,
    pub grid_spacing: f32,
    pub grid_spacing_min: f32,
    pub grid_spacing_max: f32,
    pub node_radius: f32,
    pub title_padding_x: f32,
    pub title_padding_y: f32,
    pub collapse_toggle_inset_x: f32,
    pub socket_label_inset_x: f32,
    pub socket_label_inset_y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeFileBrowserSpacing {
    pub header_height: f32,
    pub list_row_height: f32,
    pub grid_cell_width: f32,
    pub grid_cell_height: f32,
    pub grid_gap: f32,
    pub padding_x: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeCollectionSpacing {
    pub header_height: f32,
    pub item_gap: f32,
    pub padding: f32,
    pub preview_cell_width: f32,
    pub preview_cell_height: f32,
    pub preview_padding: f32,
    pub preview_caption_height: f32,
    pub minimum_preview_cell_size: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeSpacing {
    pub control: ThemeControlSpacing,
    pub box_container: ThemeBoxSpacing,
    pub property_row: ThemePropertyRowSpacing,
    pub panel: ThemePanelSpacing,
    pub menu: ThemeMenuSpacing,
    pub list: ThemeListSpacing,
    pub table: ThemeTableSpacing,
    pub editor: ThemeEditorSpacing,
    pub curve: ThemeCurveSpacing,
    pub hsv: ThemeHsvSpacing,
    pub color_ramp: ThemeColorRampSpacing,
    pub tooltip: ThemeTooltipSpacing,
    pub node_graph: ThemeNodeGraphSpacing,
    pub file_browser: ThemeFileBrowserSpacing,
    pub collection: ThemeCollectionSpacing,
}

impl Default for ThemeSpacing {
    fn default() -> Self {
        Self {
            control: ThemeControlSpacing {
                padding_x: 8.0,
                padding_y: 4.0,
                gap: 8.0,
                checkbox_label_gap: 6.0,
                error_outline_width: 2.0,
                slider_height: 32.0,
                slider_track_height: 4.0,
            },
            box_container: ThemeBoxSpacing {
                padding: 12.0,
                gap: 8.0,
            },
            property_row: ThemePropertyRowSpacing {
                label_width: 112.0,
                gap: 8.0,
                row_height: 24.0,
            },
            panel: ThemePanelSpacing {
                header_height: 24.0,
                content_padding: 8.0,
                section_gap: 8.0,
            },
            menu: ThemeMenuSpacing {
                item_height: 24.0,
                padding_x: 12.0,
                padding_y: 4.0,
                section_gap: 4.0,
                popup_padding: 4.0,
                pie_inner_inset: 19.0,
                pie_inner_radius: 12.0,
                pie_outer_radius: 88.0,
                pie_popup_diameter: 200.0,
                pie_outer_item_radius: 76.0,
                pie_nested_item_radius: 48.0,
                pie_item_max_width: 130.0,
            },
            list: ThemeListSpacing {
                row_height: 22.0,
                padding_x: 8.0,
                padding_y: 2.0,
                indent: 16.0,
                section_gap: 4.0,
            },
            table: ThemeTableSpacing {
                header_height: 26.0,
                row_height: 24.0,
                cell_padding_x: 8.0,
                cell_padding_y: 4.0,
            },
            editor: ThemeEditorSpacing {
                padding_x: 6.0,
                padding_y: 4.0,
                gutter_width: 24.0,
                toolbar_gap: 4.0,
                handle_radius: 4.0,
                grid_spacing: 16.0,
            },
            curve: ThemeCurveSpacing {
                selected_status_width: 176.0,
                interpolation_status_width: 142.0,
            },
            hsv: ThemeHsvSpacing {
                wheel_center_x: 84.0,
                wheel_center_y: 122.0,
                wheel_outer_radius: 68.0,
                wheel_inner_radius: 54.0,
                preview_right_inset: 48.0,
                preview_top_inset: 4.0,
                preview_width: 38.0,
                preview_height: 20.0,
                controls_x: 170.0,
                value_label_y: 27.0,
                value_track_y: 40.0,
                value_track_width_inset: 188.0,
                value_track_height: 14.0,
                alpha_label_y: 62.0,
                alpha_track_y: 75.0,
                alpha_track_width_inset: 238.0,
                alpha_field_y: 71.0,
                alpha_field_right_inset: 60.0,
                alpha_field_width: 50.0,
                field_cell_width: 78.0,
                field_gap: 8.0,
                field_first_row_y: 111.0,
                field_second_row_y: 143.0,
                field_hex_y: 177.0,
                field_hex_width_inset: 188.0,
                field_height: 22.0,
                field_text_inset_x: 5.0,
                field_text_inset_y: 3.0,
                help_y: 207.0,
            },
            color_ramp: ThemeColorRampSpacing {
                title_x: 8.0,
                title_y: 4.0,
                button_width: 48.0,
                button_step: 52.0,
                button_right_inset: 156.0,
                button_top_inset: 3.0,
                button_height: 20.0,
                header_hit_height: 26.0,
                button_label_x: 4.0,
                button_label_y: 3.0,
                track_x: 12.0,
                track_y: 38.0,
                track_width_inset: 24.0,
                track_height: 25.0,
                marker_line_extension: 8.0,
                marker_hit_extension: 14.0,
                track_hit_top_inset: 2.0,
                marker_handle_radius: 5.0,
                marker_handle_top_inset: 5.0,
                details_x: 8.0,
                details_y: 84.0,
                channel_x: 48.0,
                channel_y: 108.0,
                channel_step: 22.0,
                channel_width_inset: 88.0,
                channel_height: 14.0,
                channel_selection_hit_slop: 3.0,
                channel_click_hit_slop: 4.0,
                channel_label_hit_slop: 6.0,
                channel_label_x: 8.0,
                channel_label_y: 1.0,
                channel_value_right_inset: 36.0,
                editing_right_inset: 224.0,
                editing_bottom_inset: 22.0,
            },
            tooltip: ThemeTooltipSpacing {
                padding_x: 12.0,
                padding_y: 4.0,
                maximum_width: 280.0,
                viewport_margin: 4.0,
                anchor_gap: 5.0,
            },
            node_graph: ThemeNodeGraphSpacing {
                header_height: 28.0,
                header_hint_width: 210.0,
                body_padding: 8.0,
                socket_spacing: 22.0,
                socket_radius: 5.0,
                socket_hit_slop: 4.0,
                connection_tangent_min: 36.0,
                grid_spacing: 40.0,
                grid_spacing_min: 12.0,
                grid_spacing_max: 160.0,
                node_radius: 5.0,
                title_padding_x: 24.0,
                title_padding_y: 6.0,
                collapse_toggle_inset_x: 7.0,
                socket_label_inset_x: 11.0,
                socket_label_inset_y: 7.0,
            },
            file_browser: ThemeFileBrowserSpacing {
                header_height: 48.0,
                list_row_height: 22.0,
                // Matches FileBrowser's default tile width and current hitbox
                // geometry. The grid cells are 120 wide and 78 tall.
                grid_cell_width: 120.0,
                grid_cell_height: 78.0,
                grid_gap: 0.0,
                padding_x: 6.0,
            },
            collection: ThemeCollectionSpacing {
                header_height: 28.0,
                item_gap: 8.0,
                padding: 8.0,
                preview_cell_width: 112.0,
                preview_cell_height: 104.0,
                preview_padding: 5.0,
                preview_caption_height: 26.0,
                minimum_preview_cell_size: 40.0,
            },
        }
    }
}
