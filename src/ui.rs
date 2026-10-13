use crate::{
    app::{App, AppMode, ScriptRowMeta},
    config::TextAlignment,
};
use log::debug;
use ratatui::{
    prelude::*,
    text::{Line, Span, Text},
    widgets::{Clear, List, ListItem, Paragraph},
};
use std::f32::consts::PI;
use std::time::Instant;

pub fn draw(f: &mut Frame, app: &mut App) {
    debug!("Drawing frame");

    let now = Instant::now();
    app.frame_times.push_back(now);
    let cutoff = now - std::time::Duration::from_secs(1);
    while app.frame_times.front().is_some_and(|t| *t < cutoff) {
        app.frame_times.pop_front();
    }
    app.debug_fps = app.frame_times.len() as f64;
    if app.frame_times.len() >= 2 {
        app.debug_frame_ms = app.frame_times[app.frame_times.len() - 1]
            .duration_since(app.frame_times[app.frame_times.len() - 2])
            .as_secs_f64()
            * 1000.0;
    }

    let area = f.area();
    let config = &app.config;
    let general = &config.general;

    f.render_widget(Clear, area);

    let mut working_area = area;
    if config.window.is_visible() {
        let block = config.window.block(general, "");
        let inner = block.inner(area);
        f.render_widget(block, area);
        apply_section_border_colors(f, area, &config.window, general);
        working_area = inner;
    }

    if config.outer_box.is_visible() {
        let block = config.outer_box.block(general, "");
        let inner = block.inner(working_area);
        f.render_widget(block, working_area);
        apply_section_border_colors(f, working_area, &config.outer_box, general);
        working_area = inner;
    }

    let mut constraints = Vec::new();

    if app.show_debug {
        constraints.push(Constraint::Length(1));
    }

    let qst_lines = app.qst_ascii.lines().count() as u16;

    if config.qst_ascii.section.is_visible() {
        let p = &config.qst_ascii.padding;
        constraints.push(Constraint::Length(qst_lines + p.top + p.bottom));
    }

    if config.input.is_visible() {
        constraints.push(Constraint::Length(3));
    }
    if app.status_message.is_some() {
        constraints.push(Constraint::Length(3));
    }
    constraints.push(Constraint::Min(1));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(working_area);

    let mut chunk_index = 0;

    if app.show_debug {
        let chunk = chunks[chunk_index];
        chunk_index += 1;
        let debug_text = format!(
            "FPS: {:.0} | Frame: {:.1}ms | Entries: {} | Events: {}",
            app.debug_fps,
            app.debug_frame_ms,
            app.filtered_entries.len(),
            app.total_events,
        );
        let debug_widget = Paragraph::new(debug_text).style(Style::default().fg(Color::DarkGray));
        f.render_widget(debug_widget, chunk);
    }

    if config.qst_ascii.section.is_visible() {
        let chunk = chunks[chunk_index];
        chunk_index += 1;

        let p = &config.qst_ascii.padding;
        let inner_area = Rect {
            x: chunk.x + p.left,
            y: chunk.y + p.top,
            width: chunk.width.saturating_sub(p.left + p.right),
            height: chunk.height.saturating_sub(p.top + p.bottom),
        };

        let ascii_colors = parse_gradient_colors(&config.qst_ascii.gradient_colors);

        let mut widget = if ascii_colors.len() > 1 {
            let width = app
                .qst_ascii
                .lines()
                .map(|line| line.chars().count() as u16)
                .max()
                .unwrap_or(1)
                .max(1);
            let height = qst_lines.max(1);

            let lines: Vec<Line> = app
                .qst_ascii
                .lines()
                .enumerate()
                .map(|(y, line)| {
                    let spans: Vec<Span> = line
                        .chars()
                        .enumerate()
                        .map(|(x, ch)| {
                            let color = gradient_color_at_point(
                                &ascii_colors,
                                config.qst_ascii.gradient_angle,
                                x as u16,
                                y as u16,
                                width,
                                height,
                            );
                            Span::styled(ch.to_string(), Style::default().fg(color))
                        })
                        .collect();

                    if spans.is_empty() {
                        Line::from(Span::raw(""))
                    } else {
                        Line::from(spans)
                    }
                })
                .collect();
            Paragraph::new(lines)
        } else {
            let mut p_widget = Paragraph::new(app.qst_ascii.as_str());
            if let Some(color) = ascii_colors.first().copied().or_else(|| {
                config
                    .qst_ascii
                    .section
                    .fg
                    .first()
                    .and_then(|v| crate::config::parse_color(v))
            }) {
                p_widget = p_widget.style(Style::default().fg(color));
            }
            p_widget
        };

        widget = widget.alignment(
            config
                .qst_ascii
                .alignment
                .unwrap_or(crate::config::TextAlignment::Center)
                .into(),
        );
        f.render_widget(widget, inner_area);
    }

    let search_chunk = if config.input.is_visible() {
        let chunk = chunks[chunk_index];
        chunk_index += 1;
        Some(chunk)
    } else {
        None
    };

    let status_chunk = if app.status_message.is_some() {
        let chunk = chunks[chunk_index];
        chunk_index += 1;
        Some(chunk)
    } else {
        None
    };

    let list_chunk = chunks[chunk_index];

    if let Some(chunk) = search_chunk {
        let title = " Search ";
        let search_widget = Paragraph::new(app.search_query.as_str())
            .style(config.input.style())
            .block(config.input.block(general, title));
        f.render_widget(search_widget, chunk);
        apply_section_border_colors(f, chunk, &config.input, general);

        let cursor_offset = config.input.border_offset(general);
        let cursor_x = (chunk.x + cursor_offset + app.search_cursor as u16)
            .min(chunk.x + chunk.width.saturating_sub(1));
        let cursor_y = (chunk.y + cursor_offset).min(chunk.y + chunk.height.saturating_sub(1));
        f.set_cursor_position((cursor_x, cursor_y));
    } else {
        f.set_cursor_position((list_chunk.x, list_chunk.y));
    }

    if let Some(chunk) = status_chunk
        && let Some(message) = &app.status_message
    {
        let message_style = Style::default()
            .fg(Color::Yellow)
            .patch(script_meta_color_style(&app.status_message_meta));
        let status = Paragraph::new(Line::from(Span::styled(message.clone(), message_style)));
        f.render_widget(status, chunk);
    }

    let scroll_area = list_chunk;

    let padding = if config.list.section.is_visible() {
        config.list.section.border_offset(general) * 2
    } else {
        0
    };
    let entry_selected_visible = config.entry_selected.is_visible();
    let entry_style = Style::default();
    let normal_entry_style = config.entry.base_style(config.text.style());

    let entry_fg_colors = parse_gradient_colors(&config.entry.fg);
    let entry_bg_colors = parse_gradient_colors(&config.entry.bg);
    let selected_fg_colors = parse_gradient_colors(&config.entry_selected.fg);
    let selected_bg_colors = parse_gradient_colors(&config.entry_selected.bg);

    let selected_idx = app.list_state.selected();
    let highlight_symbol = if entry_selected_visible {
        config.general.highlight_symbol.as_deref().unwrap_or(">> ")
    } else {
        ""
    };
    let selected_symbol_width = highlight_symbol.chars().count() as u16;
    let mut text_area_width = scroll_area.width.saturating_sub(padding);
    text_area_width = text_area_width.saturating_sub(selected_symbol_width);
    let full_row_width = text_area_width + selected_symbol_width;

    let items: Vec<ListItem> = if app.mode == AppMode::AppSelection {
        app.filtered_entries
            .iter()
            .enumerate()
            .map(|(idx, entry)| {
                if !config.text.is_visible() {
                    return ListItem::new(Span::raw(""));
                }

                let is_fav = app.history.is_favorite(&entry.name);
                let fav_symbol = config.general.favorite_symbol.as_deref().unwrap_or("★ ");
                let empty_prefix = " ".repeat(fav_symbol.chars().count());
                let prefix = if is_fav { fav_symbol } else { &empty_prefix };
                let name_with_icon = format!("{}{}", prefix, entry.name);

                let mut display_text = aligned_text(
                    &name_with_icon,
                    text_area_width,
                    config.text.alignment(),
                    selected_symbol_width,
                );

                if entry_selected_visible {
                    let prefix = if Some(idx) == selected_idx {
                        highlight_symbol.to_string()
                    } else {
                        " ".repeat(highlight_symbol.chars().count())
                    };
                    display_text = format!("{}{}", prefix, display_text);
                }

                let item = ListItemParams {
                    config,
                    is_selected: Some(idx) == selected_idx,
                    entry: GradientColors {
                        fg: &entry_fg_colors,
                        bg: &entry_bg_colors,
                        angle: config.entry.gradient_angle,
                    },
                    selected: GradientColors {
                        fg: &selected_fg_colors,
                        bg: &selected_bg_colors,
                        angle: config.entry_selected.gradient_angle,
                    },
                    full_row_width,
                    row_style: normal_entry_style,
                    entry_style,
                    fill_row: false,
                    label_range: None,
                    label_style: Style::default(),
                };
                build_list_item(&display_text, &item)
            })
            .collect()
    } else if app.mode == AppMode::ScriptResults {
        app.script_items
            .iter()
            .enumerate()
            .map(|(idx, item)| {
                let visible_title = item.meta.display.as_deref().unwrap_or(&item.title);
                let label = visible_title.to_string();
                let alignment = if item.meta.center {
                    TextAlignment::Center
                } else {
                    config.text.alignment()
                };
                let mut display_text =
                    aligned_text(&label, text_area_width, alignment, selected_symbol_width);
                let label_range = if entry_selected_visible {
                    let prefix = if Some(idx) == selected_idx {
                        highlight_symbol.to_string()
                    } else {
                        " ".repeat(highlight_symbol.chars().count())
                    };
                    let label_start = selected_symbol_width as usize
                        + alignment_left_padding(
                            label.chars().count(),
                            text_area_width,
                            alignment,
                            selected_symbol_width,
                        );
                    display_text = format!("{}{}", prefix, display_text);
                    Some((label_start, label.chars().count()))
                } else {
                    let label_start = alignment_left_padding(
                        label.chars().count(),
                        text_area_width,
                        alignment,
                        selected_symbol_width,
                    );
                    Some((label_start, label.chars().count()))
                };
                let label_style = script_meta_color_style(&item.meta);

                let mut row_style = normal_entry_style;
                if item.meta.active {
                    row_style = row_style.patch(config.meta.active.style());
                }
                if item.meta.urgent {
                    row_style = row_style
                        .patch(config.meta.urgent.style())
                        .add_modifier(Modifier::BOLD);
                }

                let item = ListItemParams {
                    config,
                    is_selected: Some(idx) == selected_idx,
                    entry: GradientColors {
                        fg: &entry_fg_colors,
                        bg: &entry_bg_colors,
                        angle: config.entry.gradient_angle,
                    },
                    selected: GradientColors {
                        fg: &selected_fg_colors,
                        bg: &selected_bg_colors,
                        angle: config.entry_selected.gradient_angle,
                    },
                    full_row_width,
                    row_style,
                    entry_style,
                    fill_row: item.meta.active,
                    label_range,
                    label_style,
                };
                build_list_item(&display_text, &item)
            })
            .collect()
    } else {
        app.filtered_files
            .iter()
            .enumerate()
            .map(|(idx, file)| {
                let mut display_text = aligned_text(
                    file,
                    text_area_width,
                    config.text.alignment(),
                    selected_symbol_width,
                );
                if entry_selected_visible {
                    let prefix = if Some(idx) == selected_idx {
                        highlight_symbol.to_string()
                    } else {
                        " ".repeat(highlight_symbol.chars().count())
                    };
                    display_text = format!("{}{}", prefix, display_text);
                }

                let item = ListItemParams {
                    config,
                    is_selected: Some(idx) == selected_idx,
                    entry: GradientColors {
                        fg: &entry_fg_colors,
                        bg: &entry_bg_colors,
                        angle: config.entry.gradient_angle,
                    },
                    selected: GradientColors {
                        fg: &selected_fg_colors,
                        bg: &selected_bg_colors,
                        angle: config.entry_selected.gradient_angle,
                    },
                    full_row_width,
                    row_style: normal_entry_style,
                    entry_style,
                    fill_row: false,
                    label_range: None,
                    label_style: Style::default(),
                };
                build_list_item(&display_text, &item)
            })
            .collect()
    };

    let title: String = if app.mode == AppMode::AppSelection {
        config
            .list
            .apps_title
            .as_deref()
            .unwrap_or(" Applications ")
            .to_string()
    } else if app.mode == AppMode::ScriptResults {
        script_list_title(app.script_title.as_deref(), app.script_pending)
    } else {
        config
            .list
            .files_title
            .as_deref()
            .unwrap_or(" Directories ")
            .to_string()
    };
    let title_style = if app.mode == AppMode::ScriptResults {
        script_meta_color_style(&app.script_title_meta)
    } else {
        Style::default()
    };
    let title_line = Line::from(Span::styled(title, title_style));
    let mut list = List::new(items);
    if config.list.section.is_visible() {
        list = list.block(config.list.section.block_with_title(general, title_line));
    }

    f.render_stateful_widget(list, scroll_area, &mut app.list_state);
    if config.list.section.is_visible() {
        apply_section_border_colors(f, scroll_area, &config.list.section, general);
    }
}

struct GradientColors<'a> {
    fg: &'a [Color],
    bg: &'a [Color],
    angle: u16,
}

struct ListItemParams<'a> {
    config: &'a crate::config::AppConfig,
    is_selected: bool,
    entry: GradientColors<'a>,
    selected: GradientColors<'a>,
    full_row_width: u16,
    row_style: Style,
    entry_style: Style,
    fill_row: bool,
    label_range: Option<(usize, usize)>,
    label_style: Style,
}

fn script_list_title(title: Option<&str>, pending: bool) -> String {
    let title = title.unwrap_or(" Scripts ");
    if pending {
        format!("{title}…")
    } else {
        title.to_string()
    }
}

fn build_list_item(display_text: &str, item: &ListItemParams<'_>) -> ListItem<'static> {
    let entry_fg_colors = item.entry.fg;
    let entry_bg_colors = item.entry.bg;
    let entry_angle = item.entry.angle;
    let selected_fg_colors = item.selected.fg;
    let selected_bg_colors = item.selected.bg;
    let selected_angle = item.selected.angle;
    let is_selected = item.is_selected;
    let fill_row = item.fill_row;
    let full_row_width = item.full_row_width;
    let row_style = item.row_style;
    let entry_style = item.entry_style;
    let label_range = item.label_range;
    let label_style = item.label_style;
    let config = item.config;
    if !is_selected || !config.entry_selected.is_visible() {
        let rendered_text = if fill_row {
            pad_to_width(display_text, full_row_width as usize)
        } else {
            display_text.to_string()
        };

        if entry_fg_colors.len() > 1 || entry_bg_colors.len() > 1 {
            let width = rendered_text.chars().count().max(1) as u16;
            let spans: Vec<Span<'static>> = rendered_text
                .chars()
                .enumerate()
                .map(|(idx, ch)| {
                    let mut style = row_style;
                    if !entry_fg_colors.is_empty() {
                        let fg = if entry_fg_colors.len() == 1 {
                            entry_fg_colors[0]
                        } else {
                            gradient_color_at_point(
                                entry_fg_colors,
                                entry_angle,
                                idx as u16,
                                0,
                                width,
                                1,
                            )
                        };
                        style = style.fg(fg);
                    }
                    if !entry_bg_colors.is_empty() {
                        let bg = if entry_bg_colors.len() == 1 {
                            entry_bg_colors[0]
                        } else {
                            gradient_color_at_point(
                                entry_bg_colors,
                                entry_angle,
                                idx as u16,
                                0,
                                width,
                                1,
                            )
                        };
                        style = style.bg(bg);
                    }
                    if let Some((start, len)) = label_range
                        && idx >= start
                        && idx < start + len
                    {
                        style = style.patch(label_style);
                    }
                    Span::styled(ch.to_string(), style)
                })
                .collect();

            return ListItem::new(Text::from(Line::from(spans))).style(entry_style);
        }

        return ListItem::new(Text::from(Line::from(label_spans(
            &rendered_text,
            label_range,
            row_style,
            label_style,
        ))))
        .style(entry_style);
    }

    let selected_text = if config.entry_selected.full_width_highlight.unwrap_or(true) || fill_row {
        pad_to_width(display_text, full_row_width as usize)
    } else {
        display_text.to_string()
    };

    let selected_style = row_style.patch(config.entry_selected.style());
    let width = selected_text.chars().count().max(1) as u16;
    if selected_fg_colors.len() > 1 || selected_bg_colors.len() > 1 {
        let spans: Vec<Span<'static>> = selected_text
            .chars()
            .enumerate()
            .map(|(idx, ch)| {
                let mut style = selected_style;
                if !selected_fg_colors.is_empty() {
                    let fg = if selected_fg_colors.len() == 1 {
                        selected_fg_colors[0]
                    } else {
                        gradient_color_at_point(
                            selected_fg_colors,
                            selected_angle,
                            idx as u16,
                            0,
                            width,
                            1,
                        )
                    };
                    style = style.fg(fg);
                }
                if !selected_bg_colors.is_empty() {
                    let bg = if selected_bg_colors.len() == 1 {
                        selected_bg_colors[0]
                    } else {
                        gradient_color_at_point(
                            selected_bg_colors,
                            selected_angle,
                            idx as u16,
                            0,
                            width,
                            1,
                        )
                    };
                    style = style.bg(bg);
                }
                if let Some((start, len)) = label_range
                    && idx >= start
                    && idx < start + len
                {
                    style = style.patch(label_style);
                }
                Span::styled(ch.to_string(), style)
            })
            .collect();

        ListItem::new(Text::from(Line::from(spans))).style(entry_style)
    } else {
        ListItem::new(Text::from(Line::from(label_spans(
            &selected_text,
            label_range,
            selected_style,
            label_style,
        ))))
        .style(entry_style)
    }
}

fn script_meta_color_style(meta: &ScriptRowMeta) -> Style {
    let mut style = Style::default();
    if let Some(color) = meta.fg.as_deref().and_then(crate::config::parse_color) {
        style = style.fg(color);
    }
    if let Some(color) = meta.bg.as_deref().and_then(crate::config::parse_color) {
        style = style.bg(color);
    }
    style
}

fn label_spans(
    text: &str,
    range: Option<(usize, usize)>,
    base_style: Style,
    label_style: Style,
) -> Vec<Span<'static>> {
    let Some((start, len)) = range else {
        return vec![Span::styled(text.to_string(), base_style)];
    };

    let chars: Vec<char> = text.chars().collect();
    let start = start.min(chars.len());
    let end = start.saturating_add(len).min(chars.len());
    if len == 0 || label_style == Style::default() || start >= end {
        return vec![Span::styled(text.to_string(), base_style)];
    }

    let mut spans = Vec::new();
    if start > 0 {
        spans.push(Span::styled(
            chars[..start].iter().collect::<String>(),
            base_style,
        ));
    }
    spans.push(Span::styled(
        chars[start..end].iter().collect::<String>(),
        base_style.patch(label_style),
    ));
    if end < chars.len() {
        spans.push(Span::styled(
            chars[end..].iter().collect::<String>(),
            base_style,
        ));
    }
    spans
}

fn apply_section_border_colors(
    f: &mut Frame,
    area: Rect,
    section: &crate::config::SectionConfig,
    general: &crate::config::GeneralConfig,
) {
    if !section.draws_borders(general) {
        return;
    }

    let colors = parse_gradient_colors(&section.border_color);
    if colors.len() <= 1 || area.width < 2 || area.height < 2 {
        return;
    }

    let angle = section.border_angle;
    let width = area.width;
    let height = area.height;
    let right = area.x + area.width - 1;
    let bottom = area.y + area.height - 1;

    let buffer = f.buffer_mut();

    for x in area.x..=right {
        let rel_x = x - area.x;
        let top_color = gradient_color_at_point(&colors, angle, rel_x, 0, width, height);
        if let Some(cell) = buffer.cell_mut((x, area.y)) {
            cell.set_fg(top_color);
        }

        let bottom_color =
            gradient_color_at_point(&colors, angle, rel_x, height - 1, width, height);
        if let Some(cell) = buffer.cell_mut((x, bottom)) {
            cell.set_fg(bottom_color);
        }
    }

    for y in (area.y + 1)..bottom {
        let rel_y = y - area.y;
        let left_color = gradient_color_at_point(&colors, angle, 0, rel_y, width, height);
        if let Some(cell) = buffer.cell_mut((area.x, y)) {
            cell.set_fg(left_color);
        }

        let right_color = gradient_color_at_point(&colors, angle, width - 1, rel_y, width, height);
        if let Some(cell) = buffer.cell_mut((right, y)) {
            cell.set_fg(right_color);
        }
    }
}

fn parse_gradient_colors(values: &[String]) -> Vec<Color> {
    values
        .iter()
        .filter_map(|s| crate::config::parse_color(s))
        .collect()
}

fn gradient_color_at_point(
    colors: &[Color],
    angle: u16,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
) -> Color {
    if colors.is_empty() {
        return Color::White;
    }
    if colors.len() == 1 {
        return colors[0];
    }

    let factor = gradient_factor(angle, x, y, width, height);
    let segment_count = (colors.len() - 1) as f32;
    let segment_progress = factor * segment_count;
    let segment_index = segment_progress.floor() as usize;
    let segment_index = segment_index.min(colors.len() - 2);
    let local_factor = segment_progress - segment_index as f32;

    interpolate_color(
        colors[segment_index],
        colors[segment_index + 1],
        local_factor,
    )
}

fn gradient_factor(angle: u16, x: u16, y: u16, width: u16, height: u16) -> f32 {
    let max_x = width.saturating_sub(1).max(1) as f32;
    let max_y = height.saturating_sub(1).max(1) as f32;

    let nx = x as f32 / max_x;
    let ny = y as f32 / max_y;

    let radians = ((angle % 360) as f32) * PI / 180.0;
    let dx = radians.cos();
    let dy = radians.sin();

    let projected = (nx - 0.5) * dx + (ny - 0.5) * dy;
    let extent = 0.5 * (dx.abs() + dy.abs());
    if extent <= f32::EPSILON {
        return 0.0;
    }

    ((projected / extent) * 0.5 + 0.5).clamp(0.0, 1.0)
}

fn pad_to_width(text: &str, width: usize) -> String {
    let len = text.chars().count();
    if len >= width {
        text.to_string()
    } else {
        format!("{}{}", text, " ".repeat(width - len))
    }
}

fn alignment_left_padding(
    label_width: usize,
    width: u16,
    alignment: TextAlignment,
    selected_symbol_width: u16,
) -> usize {
    let width = if matches!(alignment, TextAlignment::Center) {
        width.saturating_sub(selected_symbol_width)
    } else {
        width
    } as usize;

    if label_width >= width {
        return 0;
    }

    let padding = width - label_width;
    match alignment {
        TextAlignment::Left => 0,
        TextAlignment::Right => padding,
        TextAlignment::Center => padding / 2,
    }
}

fn aligned_text(
    text: &str,
    width: u16,
    alignment: TextAlignment,
    selected_symbol_width: u16,
) -> String {
    let effective_width = if matches!(alignment, TextAlignment::Center) {
        width.saturating_sub(selected_symbol_width)
    } else {
        width
    };

    if effective_width == 0 {
        return text.to_string();
    }

    let width = effective_width as usize;
    let current = text.chars().count();
    if current >= width {
        return text.to_string();
    }

    let padding = width - current;
    match alignment {
        TextAlignment::Left => text.to_string(),
        TextAlignment::Right => format!("{:>width$}", text, width = width),
        TextAlignment::Center => {
            let left = alignment_left_padding(current, width as u16, alignment, 0);
            let right = padding - left;
            format!(
                "{left_padding}{text}{right_padding}",
                left_padding = " ".repeat(left),
                right_padding = " ".repeat(right)
            )
        }
    }
}

fn interpolate_color(c1: Color, c2: Color, factor: f32) -> Color {
    let (r1, g1, b1) = color_to_rgb(c1);
    let (r2, g2, b2) = color_to_rgb(c2);

    let r = (r1 as f32 + (r2 as f32 - r1 as f32) * factor) as u8;
    let g = (g1 as f32 + (g2 as f32 - g1 as f32) * factor) as u8;
    let b = (b1 as f32 + (b2 as f32 - b1 as f32) * factor) as u8;

    Color::Rgb(r, g, b)
}

fn color_to_rgb(c: Color) -> (u8, u8, u8) {
    match c {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Black => (0, 0, 0),
        Color::Red => (170, 0, 0),
        Color::Green => (0, 170, 0),
        Color::Yellow => (170, 85, 0),
        Color::Blue => (0, 0, 170),
        Color::Magenta => (170, 0, 170),
        Color::Cyan => (0, 170, 170),
        Color::White => (170, 170, 170),
        Color::Gray => (85, 85, 85),
        Color::DarkGray => (85, 85, 85),
        Color::LightRed => (255, 85, 85),
        Color::LightGreen => (85, 255, 85),
        Color::LightYellow => (255, 255, 85),
        Color::LightBlue => (85, 85, 255),
        Color::LightMagenta => (255, 85, 255),
        Color::LightCyan => (85, 255, 255),
        _ => (255, 255, 255),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn center_alignment_accounts_for_selection_gutter() {
        assert_eq!(
            aligned_text("abcd", 10, TextAlignment::Center, 0),
            "   abcd   "
        );
        assert_eq!(
            aligned_text("abcd", 10, TextAlignment::Center, 2),
            "  abcd  "
        );
    }

    #[test]
    fn script_list_title_marks_pending_with_ellipsis() {
        assert_eq!(script_list_title(Some(" Echo "), false), " Echo ");
        assert_eq!(script_list_title(Some(" Echo "), true), " Echo …");
        assert_eq!(script_list_title(None, false), " Scripts ");
        assert_eq!(script_list_title(None, true), " Scripts …");
    }

    #[test]
    fn alignment_left_padding_matches_aligned_text() {
        assert_eq!(alignment_left_padding(4, 10, TextAlignment::Center, 0), 3);
        assert_eq!(alignment_left_padding(4, 10, TextAlignment::Center, 2), 2);
        assert_eq!(alignment_left_padding(4, 10, TextAlignment::Left, 0), 0);
        assert_eq!(alignment_left_padding(4, 10, TextAlignment::Right, 0), 6);
        assert_eq!(alignment_left_padding(12, 10, TextAlignment::Center, 0), 0);
    }

    #[test]
    fn label_spans_style_only_the_label_region() {
        let base = Style::default().fg(Color::White);
        let label = Style::default().fg(Color::Rgb(255, 136, 0));
        let spans = label_spans("  hello  ", Some((2, 5)), base, label);

        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].content, "  ");
        assert_eq!(spans[0].style, base);
        assert_eq!(spans[1].content, "hello");
        assert_eq!(spans[1].style, base.patch(label));
        assert_eq!(spans[2].content, "  ");
        assert_eq!(spans[2].style, base);
    }

    #[test]
    fn label_spans_without_style_returns_single_span() {
        let base = Style::default().fg(Color::White);
        let spans = label_spans("hello", Some((0, 5)), base, Style::default());
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "hello");
        assert_eq!(spans[0].style, base);
    }

    #[test]
    fn script_meta_color_style_parses_fg_and_bg() {
        let meta = ScriptRowMeta {
            fg: Some("#ff8800".to_string()),
            bg: Some("blue".to_string()),
            ..ScriptRowMeta::default()
        };

        let style = script_meta_color_style(&meta);
        assert_eq!(style.fg, Some(Color::Rgb(255, 136, 0)));
        assert_eq!(style.bg, Some(Color::Blue));
    }

    #[test]
    fn script_meta_color_style_ignores_invalid_values() {
        let meta = ScriptRowMeta {
            fg: Some("not-a-color".to_string()),
            ..ScriptRowMeta::default()
        };
        assert_eq!(script_meta_color_style(&meta), Style::default());
    }
}
