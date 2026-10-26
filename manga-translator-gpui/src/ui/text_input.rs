// manga-translator-gpui/src/ui/text_input.rs
//
// Single-line text field.
//
// The editing itself (IME, dead keys, selection, clipboard, undo) comes from
// gpui-component's `InputState`; this wrapper only keeps the small API the
// panels use and turns the library's `InputEvent`s into the `on_change` /
// `on_commit` callbacks the app already relies on.

use std::rc::Rc;

use gpui::component::input::{Input, InputEvent, InputState};
use gpui::{
    App, Context, Entity, IntoElement, ParentElement as _, Render, SharedString, Styled as _,
    Window, div, prelude::*, px,
};

type ChangeFn = Rc<dyn Fn(&str, &mut Window, &mut App)>;

pub struct TextInput {
    state: Entity<InputState>,
    on_change: Option<ChangeFn>,
    on_commit: Option<ChangeFn>,
}

impl TextInput {
    pub fn new(
        cx: &mut Context<Self>,
        window: &mut Window,
        value: impl Into<SharedString>,
        placeholder: impl Into<SharedString>,
        masked: bool,
    ) -> Self {
        let value: SharedString = value.into();
        let placeholder: SharedString = placeholder.into();

        let state = cx.new(|cx| {
            let mut state = InputState::new(window, cx)
                .placeholder(placeholder)
                .masked(masked);
            if !value.is_empty() {
                state.set_value(value.to_string(), window, cx);
            }
            state
        });

        let this = Self {
            state,
            on_change: None,
            on_commit: None,
        };
        cx.subscribe_in(
            &this.state,
            window,
            |this, _input, event: &InputEvent, window, cx| match event {
                InputEvent::Change => this.notify_change(window, cx),
                InputEvent::PressEnter { .. } | InputEvent::Blur => this.commit(window, cx),
                _ => {}
            },
        )
        .detach();
        this
    }

    pub fn on_change(mut self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }

    pub fn on_commit(mut self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_commit = Some(Rc::new(f));
        self
    }

    /// Replace the content programmatically (does not fire callbacks).
    pub fn set_value(&mut self, value: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.read(cx).value().as_ref() == value {
            return;
        }
        let state = self.state.clone();
        state.update(cx, |state, cx| {
            state.set_value(value.to_string(), window, cx)
        });
    }

    fn notify_change(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(f) = self.on_change.clone() {
            let text = self.state.read(cx).value();
            f(&text, window, cx);
        }
    }

    fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(f) = self.on_commit.clone() {
            let text = self.state.read(cx).value();
            f(&text, window, cx);
        }
    }
}

impl Render for TextInput {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().w_full().child(Input::new(&self.state).h(px(26.)))
    }
}
