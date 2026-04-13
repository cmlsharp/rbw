use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use super::reducer::Action;
use super::{Mode, State};
use crate::{browser, create, generator};

pub(crate) trait StaticLabel {
    fn label(&self) -> &'static str;
}

#[macro_export]
macro_rules! keymod {
    (none) => {
        ::crossterm::event::KeyModifiers::NONE
    };
    (ctrl) => {
        ::crossterm::event::KeyModifiers::CONTROL
    };
    (shift) => {
        ::crossterm::event::KeyModifiers::SHIFT
    };
}

#[macro_export]
macro_rules! keymod_hint_text {
    (ctrl) => {
        "Ctrl"
    };
    (shift) => {
        "Shift"
    };
    ($unsupported:tt) => {
        compile_error!("unsupported modifier for binding hint text")
    };
}

#[macro_export]
macro_rules! keycode {
    (enter) => {
        ::crossterm::event::KeyCode::Enter
    };
    (esc) => {
        ::crossterm::event::KeyCode::Esc
    };
    (tab) => {
        ::crossterm::event::KeyCode::Tab
    };
    (backspace) => {
        ::crossterm::event::KeyCode::Backspace
    };
    (up) => {
        ::crossterm::event::KeyCode::Up
    };
    (down) => {
        ::crossterm::event::KeyCode::Down
    };
    (left) => {
        ::crossterm::event::KeyCode::Left
    };
    (right) => {
        ::crossterm::event::KeyCode::Right
    };
    ($ch:literal) => {
        ::crossterm::event::KeyCode::Char($ch)
    };
    ($unsupported:tt) => {
        compile_error!("unsupported key")
    };
}

#[macro_export]
macro_rules! keycode_hint_text {
    (enter) => {
        "Enter"
    };
    (esc) => {
        "Esc"
    };
    (tab) => {
        "Tab"
    };
    (backspace) => {
        "Backspace"
    };
    (up) => {
        "Up"
    };
    (down) => {
        "Down"
    };
    (left) => {
        "Left"
    };
    (right) => {
        "Right"
    };
    (' ') => {
        "Space"
    };
    ($ch:literal) => {
        const_format::formatcp!("{}", $ch)
    };
    ($unsupported:tt) => {
        compile_error!("unsupported key for binding hint text")
    };
}

#[macro_export]
macro_rules! binding_hint_text {
    (@explicit $key:tt;) => {
        $crate::binding_hint_text!(@explicit $key; none)
    };
    (@explicit $key:tt; none) => {
        const_format::concatcp!($crate::keycode_hint_text!($key))
    };
    (@explicit $key:tt; $mods:ident) => {
        const_format::concatcp!(
            "<",
            $crate::keymod_hint_text!($mods),
            "+",
            $crate::keycode_hint_text!($key),
            ">"
        )
    };
    ($mods:ident + $key:tt) => {
        $crate::binding_hint_text!(@explicit $key; $mods)
    };
    ($key:tt $(, $mods:ident)?) => {
        $crate::binding_hint_text!(@explicit $key; $($mods)?)
    };
}

#[macro_export]
macro_rules! bind {
    (@hint $key:tt; $mods:ident; false) => {
        None
    };
    (@hint $key:tt; $mods:ident; true) => {
        Some($crate::binding_hint_text!($mods + $key))
    };
    (@explicit $key:tt; $repeatable:tt; $mods:ident; $show_hint:tt; $action:expr) => {
        $crate::app::Binding::new(
            $crate::keycode!($key),
            $crate::keymod!($mods),
            $repeatable,
            $crate::bind!(@hint $key; $mods; $show_hint),
            $action,
        )
    };
    (@flags $key:tt; $mods:ident; $action:expr; $repeatable:tt; $show_hint:tt; ) => {
        $crate::bind!(@explicit $key; $repeatable; $mods; $show_hint; $action)
    };
    (@flags $key:tt; $mods:ident; $action:expr; $repeatable:tt; $show_hint:tt; repeatable; $($rest:ident;)*) => {
        $crate::bind!(@flags $key; $mods; $action; true; $show_hint; $($rest;)*)
    };
    (@flags $key:tt; $mods:ident; $action:expr; $repeatable:tt; $show_hint:tt; hint; $($rest:ident;)*) => {
        $crate::bind!(@flags $key; $mods; $action; $repeatable; true; $($rest;)*)
    };
    (@flags $key:tt; $mods:ident; $action:expr; $repeatable:tt; $show_hint:tt; $unsupported:ident; $($rest:ident;)*) => {
        compile_error!("unsupported bind option")
    };
    ($mods:ident + $key:tt => $action:expr $(, $flags:ident)*) => {
        $crate::bind!(@flags $key; $mods; $action; false; false; $($flags;)*)
    };
    ($key:tt => $action:expr $(, $flags:ident)*) => {
        $crate::bind!(@flags $key; none; $action; false; false; $($flags;)*)
    };
}

/// One concrete key binding plus its optional footer hint metadata.
#[derive(Clone, Copy)]
pub(crate) struct Binding<A> {
    code: KeyCode,
    modifiers: KeyModifiers,
    repeatable: bool,
    hint_key: Option<&'static str>,
    action: A,
}

impl<A> Binding<A> {
    pub(crate) const fn new(
        code: KeyCode,
        modifiers: KeyModifiers,
        repeatable: bool,
        hint_key: Option<&'static str>,
        action: A,
    ) -> Self {
        Self {
            code,
            modifiers,
            repeatable,
            hint_key,
            action,
        }
        .normalize()
    }

    // normalizes shift + 'c' and 'C' to shift + 'C'
    // normalizes shift + Tab and BackTab to shift + BackTab
    const fn normalize(mut self) -> Self {
        match self.code {
            KeyCode::Char(c) => {
                if c.is_ascii_uppercase() {
                    // until const traits exist, we gotta do this hack. this should just be
                    // self.modifiers.insert(KeyModifiers::SHIFT)
                    self.modifiers = KeyModifiers::from_bits_truncate(
                        self.modifiers.bits() | KeyModifiers::SHIFT.bits(),
                    );
                } else if self.modifiers.contains(KeyModifiers::SHIFT) {
                    self.code = KeyCode::Char(c.to_ascii_uppercase());
                }
            }
            KeyCode::BackTab => {
                self.modifiers = KeyModifiers::from_bits_truncate(
                    self.modifiers.bits() | KeyModifiers::SHIFT.bits(),
                );
            }
            KeyCode::Tab if self.modifiers.contains(KeyModifiers::SHIFT) => {
                self.code = KeyCode::BackTab;
            }
            _ => {}
        }
        self
    }

    pub(crate) fn matches(&self, key: KeyEvent) -> bool {
        self.code == key.code
            && self.modifiers == key.modifiers
            && match key.kind {
                KeyEventKind::Press => true,
                KeyEventKind::Repeat => self.repeatable,
                _ => false,
            }
    }

    pub(crate) fn hint_key(&self) -> Option<&'static str> {
        self.hint_key
    }

    pub(crate) fn action(&self) -> &A {
        &self.action
    }
}

/// Looks up the action bound to a key event, allowing repeat events for repeatable bindings.
pub(crate) fn lookup_action<A: Copy>(bindings: &[Binding<A>], key: KeyEvent) -> Option<A> {
    // there aren't really enough binidngs to justify a more complicated data structure, but we
    // could replace this with a map later
    bindings
        .iter()
        .find(|binding| binding.matches(key))
        .map(|binding| binding.action)
}

/// Looks up a binding first, then falls back to mode-specific text input handling.
pub(crate) fn lookup_action_with_fallback<A: Copy>(
    bindings: &[Binding<A>],
    key: KeyEvent,
    fallback: impl FnOnce(KeyEvent) -> Option<A>,
) -> Option<A> {
    lookup_action(bindings, key).or_else(|| fallback(key))
}

/// Maps one terminal key event into zero or more reducer actions.
pub(super) fn map_key_to_actions(state: &State, key: KeyEvent) -> Vec<Action> {
    match &state.mode {
        Mode::Normal => browser::map_browser_key(state, key),
        Mode::Search => browser::map_search_key(key),
        Mode::Generator(generator) => generator::map_generator_key(generator, key),
        Mode::Create(_) => create::map_create_key(key),
        Mode::DeleteConfirm(_) => browser::delete::key_map(key),
    }
}
