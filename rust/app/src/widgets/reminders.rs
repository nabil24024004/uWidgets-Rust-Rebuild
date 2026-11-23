use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_MEDIUM, DWRITE_FONT_WEIGHT_NORMAL,
    DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING,
    DWRITE_TEXT_ALIGNMENT_TRAILING,
};

use crate::i18n::tr;
use crate::models::{ReminderModel, RemindersListModel, WidgetLayout};
use crate::notes_reminders_rs;
use crate::platform::graphics::{
    draw_circle, draw_round_rect, draw_text, fill_circle, fill_round_rect, rect, rgba,
};
use crate::widgets::{EditFieldId, WidgetHitAction, WidgetRenderCtx};

/// `Reminders / List` — Responsive (`Small`, `Wide`, `Large`) interactive checklist widget.
pub fn render_list(ctx: &mut WidgetRenderCtx<'_>, layout: &WidgetLayout) {
    let mut model: RemindersListModel = layout.parse_settings();
    if ctx.is_preview && model.reminders.is_empty() {
        model.reminders = vec![
            ReminderModel {
                completed: true,
                title: "Review PRs".to_string(),
            },
            ReminderModel {
                completed: false,
                title: "Ship Rust build".to_string(),
            },
            ReminderModel {
                completed: false,
                title: "Update docs".to_string(),
            },
        ];
    }

    let tier = notes_reminders_rs::port::reminders_layout_tier(
        f64::from(ctx.width),
        f64::from(ctx.height),
    )
    .unwrap_or_else(|_| "Large".to_string());

    let flags: Vec<bool> = model.reminders.iter().map(|r| r.completed).collect();
    let remaining = notes_reminders_rs::port::count_remaining(flags.clone()).unwrap_or(0);
    let total = notes_reminders_rs::port::count_total(flags).unwrap_or(0);

    let pad = 13.0_f32;
    let w = ctx.width;
    let h = ctx.height;

    let editing_name = ctx
        .inline_edit
        .filter(|e| e.field == EditFieldId::ReminderListName);
    let list_name = if let Some(ed) = editing_name {
        format!("{}|", ed.buffer)
    } else {
        model
            .list_name
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| tr(ctx.lang, "Reminders").to_string())
    };

    if tier == "Wide" {
        // Two-column layout: left summary header (name + big remaining count), right scrollable-fit list
        let left_w = (w * 0.36).clamp(90.0, 145.0);
        let name_rect = rect(pad, pad, left_w - 6.0, pad + 26.0);
        if editing_name.is_some() {
            draw_round_rect(ctx.dc, name_rect, 5.0, ctx.accent(0.9), 1.2);
        }
        draw_text(
            ctx.gfx,
            ctx.dc,
            &list_name,
            name_rect,
            ctx.font_family,
            15.0,
            DWRITE_FONT_WEIGHT_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            ctx.accent(1.0),
            false,
        );
        ctx.push_hit(
            name_rect,
            WidgetHitAction::BeginEdit(EditFieldId::ReminderListName),
        );

        draw_text(
            ctx.gfx,
            ctx.dc,
            &remaining.to_string(),
            rect(pad, pad + 30.0, left_w - 6.0, h - pad - 18.0),
            ctx.font_family,
            34.0,
            DWRITE_FONT_WEIGHT_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            ctx.fg(1.0),
            false,
        );
        draw_text(
            ctx.gfx,
            ctx.dc,
            &format!("{remaining}/{total}"),
            rect(pad, h - pad - 18.0, left_w - 6.0, h - pad * 0.6),
            ctx.font_family,
            11.0,
            DWRITE_FONT_WEIGHT_MEDIUM,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            ctx.muted(0.85),
            false,
        );

        // Vertical divider
        fill_round_rect(
            ctx.dc,
            rect(left_w, pad, left_w + 1.0, h - pad),
            0.5,
            ctx.fg(0.12),
        );

        draw_items_column(
            ctx,
            &model.reminders,
            rect(left_w + 10.0, pad, w - pad, h - pad),
            true,
        );
    } else {
        // Small or Large: header row (title left + count right), then vertical list
        let header_h = if tier == "Small" { 22.0 } else { 28.0 };
        let name_rect = rect(pad, pad, w - pad - 38.0, pad + header_h);
        if editing_name.is_some() {
            draw_round_rect(ctx.dc, name_rect, 5.0, ctx.accent(0.9), 1.2);
        }
        draw_text(
            ctx.gfx,
            ctx.dc,
            &list_name,
            name_rect,
            ctx.font_family,
            if tier == "Small" { 14.0 } else { 17.0 },
            DWRITE_FONT_WEIGHT_BOLD,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            ctx.accent(1.0),
            false,
        );
        ctx.push_hit(
            name_rect,
            WidgetHitAction::BeginEdit(EditFieldId::ReminderListName),
        );

        draw_text(
            ctx.gfx,
            ctx.dc,
            &remaining.to_string(),
            rect(w - pad - 38.0, pad, w - pad, pad + header_h),
            ctx.font_family,
            if tier == "Small" { 15.0 } else { 19.0 },
            DWRITE_FONT_WEIGHT_BOLD,
            DWRITE_TEXT_ALIGNMENT_TRAILING,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            ctx.fg(0.95),
            false,
        );

        let list_top = pad + header_h + 6.0;
        draw_items_column(
            ctx,
            &model.reminders,
            rect(pad, list_top, w - pad, h - pad * 0.8),
            tier != "Small",
        );
    }
}

fn draw_items_column(
    ctx: &mut WidgetRenderCtx<'_>,
    items: &[ReminderModel],
    area: windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F,
    show_add_row: bool,
) {
    let avail_h = (area.bottom - area.top).max(16.0);
    let row_h = 24.0_f32;
    let add_h = if show_add_row { row_h } else { 0.0 };
    let max_rows = (((avail_h - add_h) / row_h).floor() as usize).max(1);

    let mut y = area.top;
    for (i, item) in items.iter().take(max_rows).enumerate() {
        let cy = y + row_h * 0.5;
        let check_r = 6.5_f32;
        let check_cx = area.left + check_r + 1.0;

        // Checkbox circle
        if item.completed {
            fill_circle(ctx.dc, check_cx, cy, check_r, ctx.accent(1.0));
            fill_circle(
                ctx.dc,
                check_cx,
                cy,
                check_r * 0.42,
                rgba(1.0, 1.0, 1.0, 0.95),
            );
        } else {
            draw_circle(ctx.dc, check_cx, cy, check_r, ctx.muted(0.8), 1.5);
        }
        ctx.push_hit(
            rect(
                area.left - 2.0,
                y,
                area.left + check_r * 2.0 + 6.0,
                y + row_h,
            ),
            WidgetHitAction::ToggleReminder(i),
        );

        // Title text (or active inline edit)
        let text_rect = rect(area.left + check_r * 2.0 + 8.0, y, area.right, y + row_h);
        let editing_this = ctx
            .inline_edit
            .filter(|e| e.field == EditFieldId::ReminderItem(i));
        let label = if let Some(ed) = editing_this {
            draw_round_rect(ctx.dc, text_rect, 4.0, ctx.accent(0.9), 1.0);
            format!("{}|", ed.buffer)
        } else {
            item.title.clone()
        };

        draw_text(
            ctx.gfx,
            ctx.dc,
            &label,
            text_rect,
            ctx.font_family,
            12.5,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            if item.completed {
                ctx.muted(0.55)
            } else {
                ctx.fg(0.95)
            },
            false,
        );
        ctx.push_hit(
            text_rect,
            WidgetHitAction::BeginEdit(EditFieldId::ReminderItem(i)),
        );

        // Subtle row separator
        fill_round_rect(
            ctx.dc,
            rect(text_rect.left, y + row_h - 1.0, area.right, y + row_h),
            0.5,
            ctx.fg(0.08),
        );
        y += row_h;
    }

    if show_add_row && y + 18.0 <= area.bottom + 2.0 {
        let add_rect = rect(area.left, y, area.right, (y + row_h).min(area.bottom));
        let editing_new = ctx
            .inline_edit
            .filter(|e| e.field == EditFieldId::ReminderNew);
        let label = if let Some(ed) = editing_new {
            fill_round_rect(
                ctx.dc,
                add_rect,
                5.0,
                rgba(1.0, 1.0, 1.0, if ctx.theme.is_dark { 0.08 } else { 0.30 }),
            );
            draw_round_rect(ctx.dc, add_rect, 5.0, ctx.accent(0.9), 1.0);
            format!("{}|", ed.buffer)
        } else {
            "+ New reminder".to_string()
        };
        draw_text(
            ctx.gfx,
            ctx.dc,
            &label,
            rect(
                add_rect.left + 4.0,
                add_rect.top,
                add_rect.right,
                add_rect.bottom,
            ),
            ctx.font_family,
            12.0,
            DWRITE_FONT_WEIGHT_MEDIUM,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            if editing_new.is_some() {
                ctx.fg(1.0)
            } else {
                ctx.muted(0.70)
            },
            false,
        );
        ctx.push_hit(
            add_rect,
            WidgetHitAction::BeginEdit(EditFieldId::ReminderNew),
        );
    }
}
