pub(super) fn push_unique_data_class(data_classes: &mut Vec<String>, data_class: &str) {
    if !data_classes.iter().any(|existing| existing == data_class) {
        data_classes.push(data_class.to_owned());
    }
}
