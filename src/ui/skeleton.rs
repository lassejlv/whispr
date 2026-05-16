use gpui::{InteractiveElement, IntoElement, ParentElement, Styled, div, hsla, px};
use gpui_component::h_flex;

use super::theme;

pub fn voice_box_skeleton() -> impl IntoElement {
    h_flex()
        .id("pill-loading")
        .size_full()
        .items_center()
        .justify_center()
        .child(
            h_flex()
                .gap_3()
                .px_4()
                .py_2()
                .mx_3()
                .my_3()
                .w_full()
                .h(px(52.0))
                .items_center()
                .overflow_hidden()
                .rounded_full()
                .bg(theme::pill_bg())
                .border_1()
                .border_color(theme::pill_border())
                .child(
                    div()
                        .size(px(8.0))
                        .rounded_full()
                        .bg(hsla(0.0, 0.0, 0.30, 1.0)),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(slab(118.0, 10.0))
                        .child(slab(54.0, 10.0)),
                )
                .child(
                    h_flex()
                        .ml_auto()
                        .gap_1()
                        .h(px(20.0))
                        .items_center()
                        .children([6.0, 12.0, 18.0, 10.0, 16.0, 8.0, 14.0].map(|height| {
                            div()
                                .w(px(3.0))
                                .h(px(height))
                                .rounded_sm()
                                .bg(hsla(0.0, 0.0, 1.0, 0.18))
                        })),
                ),
        )
}

fn slab(width: f32, height: f32) -> impl IntoElement {
    div()
        .w(px(width))
        .h(px(height))
        .rounded_sm()
        .bg(hsla(0.0, 0.0, 1.0, 0.14))
}
