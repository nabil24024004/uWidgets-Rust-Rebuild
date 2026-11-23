use std::collections::HashMap;
use std::sync::OnceLock;

const LOCALES_JSON: &str = include_str!("../../../assets/locales.json");

static LOCALES: OnceLock<HashMap<String, HashMap<String, String>>> = OnceLock::new();

pub const SUPPORTED_LANGUAGES: &[(&str, &str)] = &[
    ("en", "English"),
    ("de", "Deutsch"),
    ("es", "Español"),
    ("fr", "Français"),
    ("ja", "日本語"),
    ("ko", "한국어"),
    ("pl", "Polski"),
    ("ru", "Русский"),
    ("uk", "Українська"),
    ("zh-Hans", "简体中文"),
    ("zh-Hant", "繁體中文"),
];

fn map() -> &'static HashMap<String, HashMap<String, String>> {
    LOCALES.get_or_init(|| serde_json::from_str(LOCALES_JSON).unwrap_or_default())
}

pub fn tr<'a>(lang: &str, key: &'a str) -> &'a str {
    let tables = map();
    if let Some(table) = tables.get(lang)
        && let Some(val) = table.get(key)
        && !val.is_empty()
    {
        return val.as_str();
    }
    if let Some(en) = tables.get("en")
        && let Some(val) = en.get(key)
    {
        return val.as_str();
    }
    match key {
        "Clock_Flip_Title" => "Flip Clock",
        "Clock_Flip_Subtitle" => "Classic split-flap mechanical clock in Liquid Glass",
        _ => key,
    }
}

pub fn month_full(lang: &str, month1: u32) -> &'static str {
    let idx = month1.saturating_sub(1).min(11) as usize;
    match lang {
        "ru" => [
            "Январь",
            "Февраль",
            "Март",
            "Апрель",
            "Май",
            "Июнь",
            "Июль",
            "Август",
            "Сентябрь",
            "Октябрь",
            "Ноябрь",
            "Декабрь",
        ][idx],
        "de" => [
            "Januar",
            "Februar",
            "März",
            "April",
            "Mai",
            "Juni",
            "Juli",
            "August",
            "September",
            "Oktober",
            "November",
            "Dezember",
        ][idx],
        "zh-Hans" | "zh-Hant" => [
            "一月",
            "二月",
            "三月",
            "四月",
            "五月",
            "六月",
            "七月",
            "八月",
            "九月",
            "十月",
            "十一月",
            "十二月",
        ][idx],
        _ => [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ][idx],
    }
}

pub fn month_abbrev(lang: &str, month1: u32) -> &'static str {
    let idx = month1.saturating_sub(1).min(11) as usize;
    match lang {
        "ru" => [
            "янв", "фев", "мар", "апр", "май", "июн", "июл", "авг", "сен", "окт", "ноя", "дек",
        ][idx],
        "de" => [
            "Jan", "Feb", "Mär", "Apr", "Mai", "Jun", "Jul", "Aug", "Sep", "Okt", "Nov", "Dez",
        ][idx],
        "zh-Hans" | "zh-Hant" => [
            "1月", "2月", "3月", "4月", "5月", "6月", "7月", "8月", "9月", "10月", "11月", "12月",
        ][idx],
        _ => [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ][idx],
    }
}

/// Day of week 0 = Sunday .. 6 = Saturday
pub fn weekday_abbrev(lang: &str, dow0: usize) -> &'static str {
    let idx = dow0 % 7;
    match lang {
        "ru" => ["Вс", "Пн", "Вт", "Ср", "Чт", "Пт", "Сб"][idx],
        "de" => ["So", "Mo", "Di", "Mi", "Do", "Fr", "Sa"][idx],
        "zh-Hans" | "zh-Hant" => ["日", "一", "二", "三", "四", "五", "六"][idx],
        _ => ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][idx],
    }
}

pub fn weekday_shortest(lang: &str, dow0: usize) -> &'static str {
    let idx = dow0 % 7;
    match lang {
        "ru" => ["Вс", "Пн", "Вт", "Ср", "Чт", "Пт", "Сб"][idx],
        "de" => ["So", "Mo", "Di", "Mi", "Do", "Fr", "Sa"][idx],
        "zh-Hans" | "zh-Hant" => ["日", "一", "二", "三", "四", "五", "六"][idx],
        _ => ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"][idx],
    }
}

pub fn weekday_full(lang: &str, dow0: usize) -> &'static str {
    let idx = dow0 % 7;
    match lang {
        "ru" => [
            "Воскресенье",
            "Понедельник",
            "Вторник",
            "Среда",
            "Четверг",
            "Пятница",
            "Суббота",
        ][idx],
        "de" => [
            "Sonntag",
            "Montag",
            "Dienstag",
            "Mittwoch",
            "Donnerstag",
            "Freitag",
            "Samstag",
        ][idx],
        "zh-Hans" => [
            "星期日",
            "星期一",
            "星期二",
            "星期三",
            "星期四",
            "星期五",
            "星期六",
        ][idx],
        "zh-Hant" => [
            "星期日",
            "星期一",
            "星期二",
            "星期三",
            "星期四",
            "星期五",
            "星期六",
        ][idx],
        _ => [
            "Sunday",
            "Monday",
            "Tuesday",
            "Wednesday",
            "Thursday",
            "Friday",
            "Saturday",
        ][idx],
    }
}

pub fn two_letter_lang(lang: &str) -> &str {
    lang.split('-').next().unwrap_or("en")
}
