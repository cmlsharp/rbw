use crate::domain::YankTarget;
use crate::{
    app::{Binding, Context, StaticLabel},
    bind,
};

#[derive(Clone, Copy)]
pub enum Action {
    Cancel,
    ClearPrefix,
    Search,
    SearchClearAndFocus,
    SearchClear,
    SearchBackspace,
    SearchInput(char),
    FinishSearch,
    MoveDown,
    MoveUp,
    PageDown,
    PageUp,
    StartTopPrefix,
    Top,
    RevealPassword,
    ToggleFilter,
    Sync,
    Copy(YankTarget),
    YankPrefix,
    Select,
    Create,
    Delete,
    Bottom,
}

impl StaticLabel for Action {
    fn label(&self) -> &'static str {
        match self {
            Self::Cancel => "quit",
            Self::ClearPrefix => "cancel",
            Self::Search => "search",
            Self::SearchClearAndFocus | Self::SearchClear => "clear search",
            Self::SearchBackspace => "backspace",
            Self::SearchInput(_) => "type",
            Self::FinishSearch => "finish search",
            Self::MoveDown => "down",
            Self::MoveUp => "up",
            Self::PageDown => "page down",
            Self::PageUp => "page up",
            Self::StartTopPrefix | Self::Top => "top",
            Self::RevealPassword => "reveal",
            Self::ToggleFilter => "toggle filter",
            Self::Sync => "sync",
            Self::Copy(target) => target.label(),
            Self::YankPrefix => "yank",
            Self::Select => "select",
            Self::Create => "add",
            Self::Delete => "delete",
            Self::Bottom => "bottom",
        }
    }
}

const YANK_BINDINGS: &[Binding<Action>] = &[
    bind!(esc => Action::ClearPrefix, hint),
    bind!('u' => Action::Copy(YankTarget::Username), hint),
    bind!('p' => Action::Copy(YankTarget::Password), hint),
    bind!('t' => Action::Copy(YankTarget::Totp), hint),
    bind!('f' => Action::Copy(YankTarget::Folder), hint),
    bind!('U' => Action::Copy(YankTarget::Uri), hint),
    bind!('n' => Action::Copy(YankTarget::Name), hint),
    bind!('N' => Action::Copy(YankTarget::Notes), hint),
];

const TOP_BINDINGS: &[Binding<Action>] = &[
    bind!(esc => Action::ClearPrefix, hint),
    bind!('g' => Action::Top, hint),
];

const NORMAL_BINDINGS: &[Binding<Action>] = &[
    bind!('/' => Action::Search, hint),
    bind!(ctrl + backspace => Action::SearchClearAndFocus),
    bind!(down => Action::MoveDown, repeatable),
    bind!('j' => Action::MoveDown, repeatable),
    bind!(tab => Action::MoveDown, repeatable),
    bind!(up => Action::MoveUp, repeatable),
    bind!('k' => Action::MoveUp, repeatable),
    bind!(shift + tab => Action::MoveUp, repeatable),
    bind!(ctrl + 'd' => Action::PageDown, repeatable),
    bind!(ctrl + 'u' => Action::PageUp, repeatable),
    bind!('g' => Action::StartTopPrefix, hint),
    bind!('r' => Action::Sync, hint),
    bind!('y' => Action::YankPrefix, hint),
    bind!('a' => Action::Create, hint),
    bind!('d' => Action::Delete, hint),
    bind!('G' => Action::Bottom),
    bind!(ctrl + 'v' => Action::RevealPassword, hint),
    bind!('q' => Action::Cancel, hint),
    bind!(ctrl + 'c' => Action::Cancel),
];

const FILTER_BINDINGS: &[Binding<Action>] = &[bind!('t' => Action::ToggleFilter, hint)];

const SELECT_BINDINGS: &[Binding<Action>] = &[bind!(enter => Action::Select, hint)];

pub(super) fn bindings(
    context: &Context,
    state: &super::State,
) -> impl Iterator<Item = &'static Binding<Action>> {
    // generators cannot come soon enough i swear to god
    // the control flow in this is a little convoluted because i need the ultimate iterator to have
    // the same type but i need to conditionally append to it depending on stuff from context
    let primary = match state.pending {
        Some(super::state::PendingPrefix::Yank) => YANK_BINDINGS,
        Some(super::state::PendingPrefix::Top) => TOP_BINDINGS,
        None => NORMAL_BINDINGS,
    };
    let filter = state.pending.is_none() && !context.url.is_empty();
    let select = state.pending.is_none() && context.emit_output;

    [
        Some(primary),
        filter.then_some(FILTER_BINDINGS),
        select.then_some(SELECT_BINDINGS),
    ]
    .into_iter()
    .flatten()
    .flatten()
}
