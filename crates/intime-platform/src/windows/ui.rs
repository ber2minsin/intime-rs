use uiautomation::{
    UIAutomation, UIElement,
    events::{
        CustomFocusChangedEventHandler, CustomPropertyChangedEventHandler,
        UIFocusChangedEventHandler, UIPropertyChangedEventHandler,
    },
    types::{TreeScope, UIProperty},
    variants::Variant,
};

use crate::error::PlatformError;

struct FocusChangedEventHandler;

impl CustomFocusChangedEventHandler for FocusChangedEventHandler {
    fn handle(&self, sender: &uiautomation::UIElement) -> uiautomation::Result<()> {
        println!("Focus changed: {:?}", sender);
        Ok(())
    }
}
struct TextChangedHandler;

impl CustomPropertyChangedEventHandler for TextChangedHandler {
    fn handle(
        &self,
        sender: &UIElement,
        property: UIProperty,
        value: Variant,
    ) -> uiautomation::Result<()> {
        let name = sender.get_name().unwrap_or_default();
        let class = sender.get_classname().unwrap_or_default();

        println!(
            "TEXT CHANGED: {:?} {} {} -> {}",
            property, class, name, value
        );

        Ok(())
    }
}
pub fn install_hooks() -> Result<(), PlatformError> {
    let automation = UIAutomation::new().unwrap();
    // let matcher = automation.create_matcher();
    let focus_changed_handler = FocusChangedEventHandler {};
    let focus_changed_handler = UIFocusChangedEventHandler::from(focus_changed_handler);

    automation
        .add_focus_changed_event_handler(None, &focus_changed_handler)
        .unwrap();

    let root = automation.get_root_element().unwrap();

    let handler = UIPropertyChangedEventHandler::from(TextChangedHandler);

    automation.add_property_changed_event_handler(
        &root,
        TreeScope::Subtree,
        None,
        &handler,
        &[UIProperty::ValueValue],
    ).unwrap();
    Ok(())
}
