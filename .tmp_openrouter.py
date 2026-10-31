"""Replace the OpenRouter model popover with an AdwDialog + GtkListView."""

import pathlib

path = pathlib.Path("manga-translator-gtk/src/ui/settings_panel.rs")
s = path.read_text(encoding="utf-8")

# ---- 1. fields -----------------------------------------------------------
old_fields = '''    /// Popup with the sort bar and the scrollable model list.
    openrouter_popover: RefCell<Option<gtk::Popover>>,
    openrouter_list: RefCell<Option<gtk::ListBox>>,
    /// Search field above the popup list.
    openrouter_search: RefCell<Option<gtk::SearchEntry>>,'''
new_fields = '''    /// Dialog with the search field, sort bar and the virtualized model list.
    openrouter_dialog: RefCell<Option<adw::Dialog>>,
    /// Unsorted base list; the dialog's ListStore mirrors it.
    openrouter_store: RefCell<Option<gio::ListStore>>,
    openrouter_selection: RefCell<Option<gtk::SingleSelection>>,
    openrouter_list_view: RefCell<Option<gtk::ListView>>,
    /// Live filter applied to the model store.
    openrouter_filter: RefCell<Option<gtk::CustomFilter>>,
    /// Lower-cased query read by the filter closure.
    openrouter_query: Rc<RefCell<String>>,
    /// Search field above the model list.
    openrouter_search: RefCell<Option<gtk::SearchEntry>>,'''
assert old_fields in s
s = s.replace(old_fields, new_fields, 1)

old_default = '''            openrouter_popover: RefCell::new(None),
            openrouter_list: RefCell::new(None),
            openrouter_search: RefCell::new(None),'''
new_default = '''            openrouter_dialog: RefCell::new(None),
            openrouter_store: RefCell::new(None),
            openrouter_selection: RefCell::new(None),
            openrouter_list_view: RefCell::new(None),
            openrouter_filter: RefCell::new(None),
            openrouter_query: Rc::new(RefCell::new(String::new())),
            openrouter_search: RefCell::new(None),'''
assert old_default in s
s = s.replace(old_default, new_default, 1)

# ---- 2. UI block ---------------------------------------------------------
start = s.index("        let openrouter_popover = gtk::Popover::new();")
end_marker = "        openrouter_list.add_controller(motion);\n"
end = s.index(end_marker, start) + len(end_marker)

new_ui = '''        // Model picker. A dialog is the right container: 300+ entries with
        // search, sorting and a detail pane do not fit into a popover.
        let openrouter_dialog = adw::Dialog::builder()
            .content_width(760)
            .content_height(560)
            .build();

        let dialog_toolbar = adw::ToolbarView::new();
        let dialog_header = adw::HeaderBar::new();
        let dialog_title = adw::WindowTitle::new(
            &i18n::t("OpenRouter Modell"),
            &i18n::t("Modell auswählen"),
        );
        i18n::register_window_title(
            &dialog_title,
            "OpenRouter Modell",
            Some("Modell auswählen"),
        );
        dialog_header.set_title_widget(Some(&dialog_title));
        dialog_toolbar.add_top_bar(&dialog_header);

        let openrouter_search = gtk::SearchEntry::new();
        openrouter_search.set_placeholder_text(Some(&i18n::t("Modell suchen")));
        openrouter_search.set_hexpand(true);
        i18n::register_search_placeholder(&openrouter_search, "Modell suchen");
        openrouter_search.connect_search_changed(clone!(
            #[weak(rename_to = this)]
            self,
            move |entry| {
                this.filter_openrouter_list(&entry.text());
            }
        ));
        openrouter_search.connect_stop_search(clone!(
            #[weak]
            openrouter_search,
            move |_| {
                openrouter_search.set_text("");
            }
        ));

        let search_row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        search_row.set_margin_top(9);
        search_row.set_margin_bottom(3);
        search_row.set_margin_start(12);
        search_row.set_margin_end(12);
        search_row.append(&openrouter_search);

        // Sort bar. It stays above the list while the models scroll.
        let sort_bar = gtk::Box::new(gtk::Orientation::Vertical, 6);
        sort_bar.set_margin_top(9);
        sort_bar.set_margin_bottom(9);
        sort_bar.set_margin_start(9);
        sort_bar.set_margin_end(9);

        let mut sort_buttons = Vec::new();
        let mut group_anchor: Option<gtk::ToggleButton> = None;
        for chunk in OpenrouterSort::OPTIONS.chunks(2) {
            let button_row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            button_row.add_css_class("linked");
            for (sort, msgid) in chunk {
                let button = gtk::ToggleButton::with_label(&i18n::t(msgid));
                button.set_hexpand(true);
                i18n::register_toggle(&button, msgid);
                match &group_anchor {
                    Some(anchor) => button.set_group(Some(anchor)),
                    None => group_anchor = Some(button.clone()),
                }
                let panel = obj.clone();
                let sort = *sort;
                button.connect_toggled(move |button| {
                    if !button.is_active() || panel.imp().syncing_openrouter_sort.get() {
                        return;
                    }
                    panel.imp().set_openrouter_sort(sort);
                });
                button_row.append(&button);
                sort_buttons.push((sort, button));
            }
            sort_bar.append(&button_row);
        }

        // Virtualized list: only the visible rows are materialized, which keeps
        // a 300+ entry catalogue cheap.
        let openrouter_store = gio::ListStore::new::<glib::BoxedAnyObject>();
        let query = self.openrouter_query.clone();
        let openrouter_filter = gtk::CustomFilter::new(move |obj| {
            let Some(boxed) = obj.downcast_ref::<glib::BoxedAnyObject>() else {
                return false;
            };
            let model = boxed.borrow::<OpenrouterModel>();
            let query = query.borrow();
            query.is_empty()
                || model.id.to_lowercase().contains(&*query)
                || model.name.to_lowercase().contains(&*query)
        });
        let filter_model = gtk::FilterListModel::new(
            Some(openrouter_store.clone()),
            Some(openrouter_filter.clone()),
        );
        let openrouter_selection = gtk::SingleSelection::new(Some(filter_model));
        openrouter_selection.set_autoselect(false);
        openrouter_selection.set_can_unselect(true);

        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            hbox.set_margin_top(6);
            hbox.set_margin_bottom(6);
            hbox.set_margin_start(12);
            hbox.set_margin_end(12);
            let label = MarqueeLabel::new();
            label.set_hexpand(true);
            label.set_xalign(0.0);
            hbox.append(&label);
            let check = gtk::Image::from_icon_name("object-select-symbolic");
            check.set_opacity(0.0);
            hbox.append(&check);
            item.set_child(Some(&hbox));
        });
        factory.connect_bind(clone!(
            #[weak(rename_to = this)]
            self,
            move |_, item| {
                let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                    return;
                };
                let Some(model) = openrouter_model_from_item(item) else {
                    return;
                };
                let Some(hbox) = item.child().and_downcast::<gtk::Box>() else {
                    return;
                };
                if let Some(label) = hbox.first_child().and_downcast::<MarqueeLabel>() {
                    label.set_text(&model.id);
                }
                if let Some(check) = hbox.last_child().and_downcast::<gtk::Image>() {
                    let selected = *this.openrouter_selected.borrow() == model.id;
                    check.set_opacity(if selected { 1.0 } else { 0.0 });
                }
            }
        ));

        let openrouter_list_view = gtk::ListView::new(
            Some(openrouter_selection.clone()),
            Some(factory.clone()),
        );
        openrouter_list_view.set_single_click_activate(true);
        openrouter_list_view.set_vexpand(true);
        openrouter_list_view.set_hexpand(true);

        openrouter_list_view.connect_activate(clone!(
            #[weak(rename_to = this)]
            self,
            #[weak]
            openrouter_dialog,
            move |view, position| {
                let Some(selection) = view.model().and_downcast::<gtk::SingleSelection>() else {
                    return;
                };
                let Some(model) = openrouter_model_from_selection(&selection, position) else {
                    return;
                };
                this.accept_openrouter_model(model);
                openrouter_dialog.close();
            }
        ));

        openrouter_selection.connect_selected_notify(clone!(
            #[weak(rename_to = this)]
            self,
            move |selection| {
                let model = selection
                    .selected_item()
                    .and_downcast::<glib::BoxedAnyObject>()
                    .map(|obj| obj.borrow::<OpenrouterModel>().clone());
                this.update_openrouter_details_for(model);
            }
        ));

        // Details of the selected model.
        let details_panel = gtk::Box::new(gtk::Orientation::Vertical, 8);
        details_panel.set_size_request(150, -1);
        details_panel.set_halign(gtk::Align::Fill);
        details_panel.set_margin_top(9);
        details_panel.set_margin_bottom(9);
        details_panel.set_margin_start(12);
        details_panel.set_margin_end(12);

        let openrouter_details_title = gtk::Label::new(None);
        openrouter_details_title.set_xalign(0.0);
        openrouter_details_title.set_wrap(true);
        openrouter_details_title.add_css_class("heading");
        details_panel.append(&openrouter_details_title);

        let openrouter_details_input = gtk::Label::new(None);
        let openrouter_details_output = gtk::Label::new(None);
        let openrouter_details_intelligence = gtk::Label::new(None);
        for (msgid, value) in [
            ("Input-Preis", &openrouter_details_input),
            ("Output-Preis", &openrouter_details_output),
            ("Intelligenz-Index", &openrouter_details_intelligence),
        ] {
            details_panel.append(&detail_block(msgid, value));
        }

        let content_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        content_box.set_vexpand(true);
        content_box.append(&openrouter_list_view);
        let details_separator = gtk::Separator::new(gtk::Orientation::Vertical);
        details_separator.set_margin_top(6);
        details_separator.set_margin_bottom(6);
        content_box.append(&details_separator);
        content_box.append(&details_panel);

        // Narrow dialogs stack the details below the list.
        let narrow = adw::Breakpoint::new(
            adw::BreakpointCondition::parse("max-width: 640px")
                .expect("valid breakpoint condition"),
        );
        narrow.add_setter(
            &content_box,
            "orientation",
            Some(&gtk::Orientation::Vertical.to_value()),
        );
        narrow.add_setter(
            &details_separator,
            "orientation",
            Some(&gtk::Orientation::Horizontal.to_value()),
        );
        openrouter_dialog.add_breakpoint(narrow);

        let dialog_content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        dialog_content.append(&search_row);
        dialog_content.append(&sort_bar);
        dialog_content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        dialog_content.append(&content_box);
        dialog_toolbar.set_content(Some(&dialog_content));
        openrouter_dialog.set_child(Some(&dialog_toolbar));

        openrouter_model_row.connect_activated(clone!(
            #[weak(rename_to = this)]
            self,
            #[weak]
            openrouter_dialog,
            move |_| {
                let root = this.obj().root();
                openrouter_dialog.present(root.as_ref());
                if let Some(view) = this.openrouter_list_view.borrow().as_ref() {
                    view.grab_focus();
                }
            }
        ));

        // A stale filter would hide the list the next time the dialog opens.
        openrouter_dialog.connect_closed(clone!(
            #[weak]
            openrouter_search,
            move |_| {
                openrouter_search.set_text("");
            }
        ));
'''
s = s[:start] + new_ui + s[end:]

# ---- 3. methods ----------------------------------------------------------
start = s.index("    /// Rebuild the popup rows from `openrouter_models` in the current order.")
end_marker = "    /// Write the selected model id onto the row.\n"
end = s.index(end_marker, start)

new_methods = '''    /// Rebuild the list store from `openrouter_models` in the current order.
    ///
    /// With `fallback_to_first`, a list without a configured model selects the
    /// first entry so a translation always has a model to send.
    fn rebuild_openrouter_store(&self, fallback_to_first: bool) {
        let mut models = self.openrouter_models.borrow().clone();
        self.openrouter_sort.get().sort(&mut models);
        *self.openrouter_models.borrow_mut() = models.clone();

        if let Some(store) = self.openrouter_store.borrow().as_ref() {
            let items: Vec<glib::BoxedAnyObject> =
                models.iter().cloned().map(glib::BoxedAnyObject::new).collect();
            store.splice(0, store.n_items(), items);
        }

        let mut selected = self.openrouter_selected.borrow().clone();
        let mut index = models.iter().position(|model| model.id == selected);
        if index.is_none() && fallback_to_first && !models.is_empty() {
            index = Some(0);
            selected = models[0].id.clone();
            *self.openrouter_selected.borrow_mut() = selected.clone();
        }

        if let Some(selection) = self.openrouter_selection.borrow().as_ref() {
            select_openrouter_by_id(selection, &selected);
        }

        self.update_openrouter_details_for(index.and_then(|index| models.get(index).cloned()));
        self.update_openrouter_value_label();

        // A filter stays active when the list is rebuilt (sort or fetch).
        if let Some(search) = self.openrouter_search.borrow().as_ref() {
            self.filter_openrouter_list(&search.text());
        }
    }

    /// Show only the models whose id or name contains *query*.
    fn filter_openrouter_list(&self, query: &str) {
        *self.openrouter_query.borrow_mut() = query.trim().to_lowercase();
        if let Some(filter) = self.openrouter_filter.borrow().as_ref() {
            filter.changed(gtk::FilterChange::Different);
        }
    }

'''
s = s[:start] + new_methods + s[end:]

# update_openrouter_details -> update_openrouter_details_for
old_details_start = s.index("    /// Show the numbers of the given model in the popup's detail sidebar.\n    fn update_openrouter_details(")
old_details_end_marker = "    /// Select a model from the popup and persist it.\n"
old_details_end = s.index(old_details_end_marker, old_details_start)

new_details = '''    /// Show the numbers of the given model in the detail sidebar.
    fn update_openrouter_details_for(&self, model: Option<OpenrouterModel>) {
        let (title, input, output, intelligence) = match model {
            Some(model) => (
                model.id,
                format_token_price(model.price),
                format_token_price(model.completion),
                format_score(model.intelligence),
            ),
            None => (
                String::new(),
                "—".to_string(),
                "—".to_string(),
                "—".to_string(),
            ),
        };

        if let Some(label) = self.openrouter_details_title.borrow().as_ref() {
            label.set_text(&title);
            label.set_tooltip_text(if title.is_empty() {
                None
            } else {
                Some(title.as_str())
            });
        }
        if let Some(label) = self.openrouter_details_input.borrow().as_ref() {
            label.set_text(&input);
        }
        if let Some(label) = self.openrouter_details_output.borrow().as_ref() {
            label.set_text(&output);
        }
        if let Some(label) = self.openrouter_details_intelligence.borrow().as_ref() {
            label.set_text(&intelligence);
        }
    }

'''
s = s[:old_details_start] + new_details + s[old_details_end:]

old_accept_start = s.index("    /// Select a model from the popup and persist it.\n    fn on_openrouter_row_activated(")
old_accept_end_marker = "    fn on_local_model_changed(&self) {"
old_accept_end = s.index(old_accept_end_marker, old_accept_start)

new_accept = '''    /// Accept a model from the dialog and persist it.
    fn accept_openrouter_model(&self, model: OpenrouterModel) {
        *self.openrouter_selected.borrow_mut() = model.id.clone();
        self.update_openrouter_value_label();

        if let Some(cfg) = self.config.borrow().as_ref() {
            cfg.borrow_mut().settings.openrouter_model = model.id;
            cfg.borrow().save_settings();
        }

        self.fire_setting_changed(SettingKind::OpenrouterModel);
    }

'''
s = s[:old_accept_start] + new_accept + s[old_accept_end:]

# ---- 4. storage ----------------------------------------------------------
old_store = '''        *self.openrouter_popover.borrow_mut() = Some(openrouter_popover);
        *self.openrouter_list.borrow_mut() = Some(openrouter_list);
        *self.openrouter_search.borrow_mut() = Some(openrouter_search);'''
new_store = '''        *self.openrouter_dialog.borrow_mut() = Some(openrouter_dialog);
        *self.openrouter_store.borrow_mut() = Some(openrouter_store);
        *self.openrouter_selection.borrow_mut() = Some(openrouter_selection);
        *self.openrouter_list_view.borrow_mut() = Some(openrouter_list_view);
        *self.openrouter_filter.borrow_mut() = Some(openrouter_filter);
        *self.openrouter_search.borrow_mut() = Some(openrouter_search);'''
assert old_store in s
s = s.replace(old_store, new_store, 1)

# ---- 5. call sites -------------------------------------------------------
s = s.replace("rebuild_openrouter_list(", "rebuild_openrouter_store(")

# ---- 6. helpers ----------------------------------------------------------
helper_marker = "fn format_token_price("
helper = '''/// Extract the model carried by a virtualized list item.
fn openrouter_model_from_item(item: &gtk::ListItem) -> Option<OpenrouterModel> {
    item.item()
        .and_downcast::<glib::BoxedAnyObject>()
        .map(|obj| obj.borrow::<OpenrouterModel>().clone())
}

/// Read the model at *position* from a single-selection model.
fn openrouter_model_from_selection(
    selection: &gtk::SingleSelection,
    position: u32,
) -> Option<OpenrouterModel> {
    selection
        .item(position)
        .and_downcast::<glib::BoxedAnyObject>()
        .map(|obj| obj.borrow::<OpenrouterModel>().clone())
}

/// Select the row whose model id is *id*, if it is currently visible.
fn select_openrouter_by_id(selection: &gtk::SingleSelection, id: &str) {
    let Some(model) = selection.model() else {
        return;
    };
    for position in 0..model.n_items() {
        let Some(obj) = model.item(position) else {
            continue;
        };
        let Some(boxed) = obj.downcast_ref::<glib::BoxedAnyObject>() else {
            continue;
        };
        if boxed.borrow::<OpenrouterModel>().id == id {
            selection.set_selected(position);
            return;
        }
    }
}

'''
assert helper_marker in s
s = s.replace(helper_marker, helper + helper_marker, 1)

path.write_text(s, encoding="utf-8")
print("rewritten")
