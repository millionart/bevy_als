use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

#[derive(Resource, Default, Clone, PartialEq)]
pub(super) struct ColumnNavigation {
    pub selected: usize,
    pub max_columns: usize,
    pub last_columns: usize,
    pub offset: f32,
}

#[derive(Clone, PartialEq)]
pub(super) struct HostLayout {
    pub lines: Vec<(String, Color, Vec2)>,
    pub actor_y: f32,
}

pub(super) fn host_layout(
    metrics: super::CanvasMetrics,
    clip_y: f32,
    target_name: &str,
    body_height: f32,
    diagnostics: &[(String, Color)],
    navigation: &mut ColumnNavigation,
    measure: &mut TextMeasure,
) -> HostLayout {
    let step = super::HOST_MAX_CHAR_HEIGHT * 1.15 * metrics.host_scale;
    let mut cursor = DebugCursor::new(Vec2::new(4.0, 50.0) * metrics.canvas_to_ui, step, clip_y);
    let mut lines = Vec::new();
    let font = super::HOST_FONT_PX * metrics.host_scale;
    let mut measured = true;
    for text in [format!("Showing Debug for {target_name}, Press [PageUp] and [PageDown] to cycle between targets."),
        "Press [Right Shift] or [Left Shift] to move the debug menu a column over to see more of the debug menu".to_owned()] {
        let size = measure.size(&text, font).unwrap_or_else(|| { measured = false; Vec2::new(0.0, step) });
        let position = cursor.line(size, navigation);
        lines.push((text, Color::linear_rgb(1.0, 0.0, 0.0), position));
    }
    let actor_y = cursor.position.y;
    cursor.position.y += body_height;
    if measured {
        for (text, color) in diagnostics {
            if let Some(size) = measure.size(text, font) {
                lines.push((text.clone(), *color, cursor.line(size, navigation)));
            }
        }
    }
    HostLayout { lines, actor_y }
}

pub(super) struct DebugCursor {
    pub position: Vec2,
    initial_y: f32,
    next_x: f32,
    step: f32,
    clip_y: f32,
    column: usize,
}

impl DebugCursor {
    pub fn new(position: Vec2, step: f32, clip_y: f32) -> Self {
        Self {
            position,
            initial_y: position.y,
            next_x: 0.0,
            step,
            clip_y,
            column: 0,
        }
    }

    pub fn line(&mut self, size: Vec2, navigation: &mut ColumnNavigation) -> Vec2 {
        let position = self.position - Vec2::X * navigation.offset;
        self.next_x = self.next_x.max(self.position.x + size.x);
        self.position.y += size.y.max(self.step);
        if self.position.y + self.step > self.clip_y {
            self.position = Vec2::new(self.next_x + self.step * 2.0, self.initial_y);
            if self.column == 0 {
                navigation.max_columns = navigation.last_columns;
            }
            self.column += 1;
            navigation.last_columns = self.column;
            if navigation.selected == self.column {
                navigation.offset = self.position.x;
            }
            if navigation.selected == 0 {
                navigation.offset = 0.0;
            }
        }
        position
    }
}

#[derive(SystemParam)]
pub(super) struct TextMeasure<'w> {
    fonts: Option<Res<'w, Assets<Font>>>,
    pipeline: Option<ResMut<'w, bevy::text::TextPipeline>>,
    font_cx: Option<ResMut<'w, bevy::text::FontCx>>,
    layout_cx: Option<ResMut<'w, bevy::text::LayoutCx>>,
    rem: Option<Res<'w, bevy::text::RemSize>>,
}

impl TextMeasure<'_> {
    pub fn size(&mut self, text: &str, font_px: f32) -> Option<Vec2> {
        let font = TextFont {
            font_size: FontSize::Px(font_px),
            weight: FontWeight::LIGHT,
            ..default()
        };
        let mut computed = bevy::text::ComputedTextBlock::default();
        self.pipeline
            .as_mut()?
            .create_text_measure(
                Entity::PLACEHOLDER,
                self.fonts.as_ref()?,
                std::iter::once((
                    Entity::PLACEHOLDER,
                    0,
                    text,
                    &font,
                    Color::WHITE,
                    bevy::text::LineHeight::default(),
                    bevy::text::LetterSpacing::default(),
                )),
                1.0,
                &TextLayout::no_wrap(),
                &mut computed,
                self.font_cx.as_mut()?,
                self.layout_cx.as_mut()?,
                Vec2::ZERO,
                self.rem.as_ref()?.0,
            )
            .ok()?;
        Some(Vec2::new(
            computed.buffer().full_width(),
            computed.buffer().height(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_cursor_wraps_after_drawing_and_updates_column_offset() {
        let mut navigation = ColumnNavigation {
            selected: 1,
            last_columns: 1,
            ..default()
        };
        let mut cursor = DebugCursor::new(Vec2::new(4.0, 50.0), 15.0, 100.0);
        assert_eq!(
            cursor.line(Vec2::new(80.0, 13.0), &mut navigation),
            Vec2::new(4.0, 50.0)
        );
        cursor.position.y = 180.0;
        assert_eq!(
            cursor.line(Vec2::new(20.0, 13.0), &mut navigation),
            Vec2::new(4.0, 180.0)
        );
        assert_eq!(cursor.position, Vec2::new(114.0, 50.0));
        assert_eq!(navigation.max_columns, 1);
        assert_eq!(navigation.offset, 114.0);
        assert_eq!(
            cursor.line(Vec2::new(20.0, 13.0), &mut navigation),
            Vec2::new(0.0, 50.0)
        );
    }
}
