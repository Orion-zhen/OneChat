use gpui::{AnyElement, App, ElementId, Hsla, div, prelude::*, px};
use gpui_component::{
    ActiveTheme as _, Icon as ComponentIcon, IconName,
    button::{Button, ButtonVariants as _},
};
use lucide_icons::Icon as LucideIcon;

pub(crate) const LUCIDE_FONT_FAMILY: &str = "lucide";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AppIcon {
    Archive,
    ArrowDown,
    ArrowUp,
    At,
    AudioLines,
    Bot,
    Braces,
    Brain,
    ChevronDown,
    ChevronLeft,
    ChevronRight,
    ChevronUp,
    Close,
    Command,
    Compose,
    Continue,
    ContextSelect,
    ContextSelected,
    Copy,
    Eye,
    Export,
    FileDown,
    FileText,
    FileUp,
    Fork,
    Image,
    Info,
    Key,
    Layers,
    Maximize,
    MessageText,
    Mic,
    Minimize,
    Pause,
    Pencil,
    Play,
    Pin,
    Plug,
    Plus,
    Regenerate,
    Save,
    Search,
    Settings,
    Shapes,
    Sidebar,
    Sliders,
    Sparkles,
    Stop,
    Temporary,
    Trash,
}

#[derive(Clone)]
enum IconBackend {
    Component(IconName),
    Lucide(LucideIcon),
    SolidSquare,
}

impl AppIcon {
    fn backend(self) -> IconBackend {
        match self {
            Self::Archive => IconBackend::Lucide(LucideIcon::Archive),
            Self::ArrowDown => IconBackend::Component(IconName::ArrowDown),
            Self::ArrowUp => IconBackend::Component(IconName::ArrowUp),
            Self::At => IconBackend::Lucide(LucideIcon::AtSign),
            Self::AudioLines => IconBackend::Lucide(LucideIcon::AudioLines),
            Self::Bot => IconBackend::Lucide(LucideIcon::Bot),
            Self::Braces => IconBackend::Lucide(LucideIcon::Braces),
            Self::Brain => IconBackend::Lucide(LucideIcon::Brain),
            Self::ChevronDown => IconBackend::Component(IconName::ChevronDown),
            Self::ChevronLeft => IconBackend::Component(IconName::ChevronLeft),
            Self::ChevronRight => IconBackend::Component(IconName::ChevronRight),
            Self::ChevronUp => IconBackend::Component(IconName::ChevronUp),
            Self::Close => IconBackend::Component(IconName::Close),
            Self::Command => IconBackend::Lucide(LucideIcon::Command),
            Self::Compose => IconBackend::Lucide(LucideIcon::MessageSquarePlus),
            Self::Continue => IconBackend::Lucide(LucideIcon::ArrowRight),
            Self::ContextSelect => IconBackend::Lucide(LucideIcon::MessageCircleCheck),
            Self::ContextSelected => IconBackend::Component(IconName::CircleCheck),
            Self::Copy => IconBackend::Component(IconName::Copy),
            Self::Eye => IconBackend::Lucide(LucideIcon::Eye),
            Self::Export => IconBackend::Lucide(LucideIcon::Share),
            Self::FileDown => IconBackend::Lucide(LucideIcon::FileDown),
            Self::FileText => IconBackend::Lucide(LucideIcon::FileText),
            Self::FileUp => IconBackend::Lucide(LucideIcon::FileUp),
            Self::Fork => IconBackend::Lucide(LucideIcon::GitFork),
            Self::Image => IconBackend::Lucide(LucideIcon::Image),
            Self::Info => IconBackend::Component(IconName::Info),
            Self::Key => IconBackend::Lucide(LucideIcon::KeyRound),
            Self::Layers => IconBackend::Lucide(LucideIcon::Layers),
            Self::Maximize => IconBackend::Lucide(LucideIcon::Maximize),
            Self::MessageText => IconBackend::Lucide(LucideIcon::MessageSquareText),
            Self::Mic => IconBackend::Lucide(LucideIcon::Mic),
            Self::Minimize => IconBackend::Lucide(LucideIcon::Minimize),
            Self::Pause => IconBackend::Lucide(LucideIcon::Pause),
            Self::Pencil => IconBackend::Lucide(LucideIcon::Pencil),
            Self::Play => IconBackend::Lucide(LucideIcon::Play),
            Self::Pin => IconBackend::Lucide(LucideIcon::Pin),
            Self::Plug => IconBackend::Lucide(LucideIcon::PlugZap),
            Self::Plus => IconBackend::Component(IconName::Plus),
            Self::Regenerate => IconBackend::Lucide(LucideIcon::RefreshCw),
            Self::Save => IconBackend::Lucide(LucideIcon::Save),
            Self::Search => IconBackend::Lucide(LucideIcon::Search),
            Self::Settings => IconBackend::Component(IconName::Settings),
            Self::Shapes => IconBackend::Lucide(LucideIcon::Shapes),
            Self::Sidebar => IconBackend::Lucide(LucideIcon::PanelLeft),
            Self::Sliders => IconBackend::Lucide(LucideIcon::SlidersHorizontal),
            Self::Sparkles => IconBackend::Lucide(LucideIcon::Sparkles),
            Self::Stop => IconBackend::SolidSquare,
            Self::Temporary => IconBackend::Lucide(LucideIcon::MessageCircleDashed),
            Self::Trash => IconBackend::Lucide(LucideIcon::Trash2),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IconTone {
    Foreground,
    Muted,
    Accent,
    Danger,
    Success,
    Warning,
    OnAccent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum IconActionSize {
    Compact,
    Regular,
}

impl IconActionSize {
    pub(super) fn icon_action(
        self,
        id: impl Into<ElementId>,
        icon: AppIcon,
        tone: IconTone,
        tooltip: &'static str,
        cx: &App,
    ) -> Button {
        let (button_size, icon_size) = match self {
            Self::Compact => (30.0, 16.0),
            Self::Regular => (36.0, 19.0),
        };
        Button::new(id)
            .ghost()
            .tooltip(tooltip)
            .size(px(button_size))
            .p_0()
            .child(render_icon(icon, tone, icon_size, cx))
    }

    pub(super) fn primary_icon_action(
        self,
        id: impl Into<ElementId>,
        icon: AppIcon,
        tooltip: &'static str,
        cx: &App,
    ) -> Button {
        self.icon_action(id, icon, IconTone::OnAccent, tooltip, cx)
            .primary()
    }

    pub(super) fn danger_icon_action(
        self,
        id: impl Into<ElementId>,
        icon: AppIcon,
        tooltip: &'static str,
        cx: &App,
    ) -> Button {
        self.icon_action(id, icon, IconTone::OnAccent, tooltip, cx)
            .danger()
    }
}

pub(super) fn selected_check_badge(cx: &App) -> AnyElement {
    div()
        .flex_none()
        .size(px(28.0))
        .rounded_full()
        .bg(cx.theme().accent)
        .flex()
        .items_center()
        .justify_center()
        .child(
            ComponentIcon::new(IconName::Check)
                .size(px(16.0))
                .text_color(cx.theme().primary),
        )
        .into_any_element()
}

pub(crate) fn render_icon(icon: AppIcon, tone: IconTone, size: f32, cx: &App) -> AnyElement {
    let color = icon_color(tone, cx);
    match icon.backend() {
        IconBackend::Component(icon) => ComponentIcon::new(icon)
            .size(px(size))
            .text_color(color)
            .into_any_element(),
        IconBackend::Lucide(icon) => div()
            .size(px(size))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .font_family(LUCIDE_FONT_FAMILY)
            .text_size(px(size))
            .line_height(px(size))
            .text_color(color)
            .child(char::from(icon).to_string())
            .into_any_element(),
        IconBackend::SolidSquare => div()
            .size(px(size))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .child(div().size(px(size * 0.625)).rounded(px(1.5)).bg(color))
            .into_any_element(),
    }
}

fn icon_color(tone: IconTone, cx: &App) -> Hsla {
    match tone {
        IconTone::Foreground => cx.theme().foreground,
        IconTone::Muted => cx.theme().muted_foreground,
        IconTone::Accent => cx.theme().primary,
        IconTone::Danger => cx.theme().danger,
        IconTone::Success => cx.theme().success,
        IconTone::Warning => cx.theme().warning,
        IconTone::OnAccent => cx.theme().primary_foreground,
    }
}
