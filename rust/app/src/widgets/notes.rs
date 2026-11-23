use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
    DWRITE_PARAGRAPH_ALIGNMENT_NEAR, DWRITE_TEXT_ALIGNMENT_LEADING,
};

use crate::i18n::tr;
use crate::models::{NoteModel, WidgetLayout};
use crate::platform::graphics::{draw_round_rect, draw_text, fill_round_rect, rect, rgba};
use crate::widgets::{EditFieldId, WidgetHitAction, WidgetRenderCtx};

/// `Notes / Note` — Sticky note with inline click-through editable Title + multi-line Content.
pub fn render_note(ctx: &mut WidgetRenderCtx<'_>, layout: &WidgetLayout) {
    let settings: NoteModel = layout.parse_settings();
    let pad = 14.0_f32;
    let w = ctx.width;
    let h = ctx.height;

    let editing_title = ctx
        .inline_edit
        .filter(|e| e.field == EditFieldId::NoteTitle);
    let editing_content = ctx
        .inline_edit
        .filter(|e| e.field == EditFieldId::NoteContent);

    let title_str = if let Some(ed) = editing_title {
        format!("{}|", ed.buffer)
    } else if let Some(t) = settings.title.as_deref().filter(|s| !s.is_empty()) {
        t.to_string()
    } else {
        tr(ctx.lang, "Notes_Title").to_string()
    };

    let content_str = if let Some(ed) = editing_content {
        format!("{}|", ed.buffer)
    } else if let Some(c) = settings.content.as_deref().filter(|s| !s.is_empty()) {
        c.to_string()
    } else if ctx.is_preview {
        "Double-click to edit note…".to_string()
    } else {
        "Double-click to write…".to_string()
    };

    let title_is_placeholder =
        editing_title.is_none() && settings.title.as_deref().is_none_or(str::is_empty);
    let content_is_placeholder =
        editing_content.is_none() && settings.content.as_deref().is_none_or(str::is_empty);

    // Header band accent bar + title
    let title_h = 26.0_f32;
    let title_rect = rect(pad, pad, w - pad, pad + title_h);
    if editing_title.is_some() {
        fill_round_rect(
            ctx.dc,
            rect(
                title_rect.left - 4.0,
                title_rect.top - 2.0,
                title_rect.right + 4.0,
                title_rect.bottom + 2.0,
            ),
            6.0,
            rgba(1.0, 1.0, 1.0, if ctx.theme.is_dark { 0.08 } else { 0.35 }),
        );
        draw_round_rect(
            ctx.dc,
            rect(
                title_rect.left - 4.0,
                title_rect.top - 2.0,
                title_rect.right + 4.0,
                title_rect.bottom + 2.0,
            ),
            6.0,
            ctx.accent(0.9),
            1.2,
        );
    }

    draw_text(
        ctx.gfx,
        ctx.dc,
        &title_str,
        title_rect,
        ctx.font_family,
        16.0,
        DWRITE_FONT_WEIGHT_BOLD,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
        if title_is_placeholder {
            ctx.accent(0.85)
        } else {
            ctx.accent(1.0)
        },
        false,
    );
    ctx.push_hit(
        title_rect,
        WidgetHitAction::BeginEdit(EditFieldId::NoteTitle),
    );

    // Subtle divider line
    let div_y = pad + title_h + 6.0;
    fill_round_rect(
        ctx.dc,
        rect(pad, div_y, w - pad, div_y + 1.0),
        0.5,
        ctx.fg(0.14),
    );

    // Optional Updated timestamp at bottom
    let footer_h = if settings.updated.is_some() && h >= 130.0 {
        16.0
    } else {
        0.0
    };

    // Multi-line content area
    let content_rect = rect(
        pad,
        div_y + 7.0,
        w - pad,
        (h - pad - footer_h).max(div_y + 24.0),
    );
    if editing_content.is_some() {
        fill_round_rect(
            ctx.dc,
            rect(
                content_rect.left - 4.0,
                content_rect.top - 2.0,
                content_rect.right + 4.0,
                content_rect.bottom + 2.0,
            ),
            6.0,
            rgba(1.0, 1.0, 1.0, if ctx.theme.is_dark { 0.08 } else { 0.35 }),
        );
        draw_round_rect(
            ctx.dc,
            rect(
                content_rect.left - 4.0,
                content_rect.top - 2.0,
                content_rect.right + 4.0,
                content_rect.bottom + 2.0,
            ),
            6.0,
            ctx.accent(0.9),
            1.2,
        );
    }
    draw_text(
        ctx.gfx,
        ctx.dc,
        &content_str,
        content_rect,
        ctx.font_family,
        13.0,
        DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_TEXT_ALIGNMENT_LEADING,
        DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
        if content_is_placeholder {
            ctx.muted(0.75)
        } else {
            ctx.fg(0.95)
        },
        true,
    );
    ctx.push_hit(
        content_rect,
        WidgetHitAction::BeginEdit(EditFieldId::NoteContent),
    );

    if footer_h > 0.0
        && let Some(updated) = settings.updated.as_deref()
    {
        let short = updated.replace('T', " ");
        let short_disp = if short.len() > 16 {
            &short[..16]
        } else {
            &short
        };
        draw_text(
            ctx.gfx,
            ctx.dc,
            short_disp,
            rect(pad, h - pad - footer_h + 2.0, w - pad, h - pad * 0.5),
            ctx.font_family,
            10.0,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            ctx.muted(0.65),
            false,
        );
    }
}
