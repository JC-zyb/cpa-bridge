const ACCOUNT_LIST_PAGE_SIZE: usize = 100;

pub fn build_account_list_query(page: usize) -> Vec<(&'static str, String)> {
    vec![
        ("page", page.to_string()),
        ("page_size", ACCOUNT_LIST_PAGE_SIZE.to_string()),
        ("lite", String::from("1")),
    ]
}

pub fn should_continue_account_list_paging(current_page_items: usize) -> bool {
    current_page_items > 0
}

#[cfg(test)]
mod tests {
    use super::{build_account_list_query, should_continue_account_list_paging};

    #[test]
    fn build_account_list_query_omits_platform_and_type_filters() {
        let query = build_account_list_query(3);
        assert_eq!(query.len(), 3);
        assert_eq!(query[0], ("page", String::from("3")));
        assert_eq!(query[1], ("page_size", String::from("100")));
        assert_eq!(query[2], ("lite", String::from("1")));
    }

    #[test]
    fn should_continue_account_list_paging_until_page_is_empty() {
        assert!(should_continue_account_list_paging(100));
        assert!(should_continue_account_list_paging(13));
        assert!(!should_continue_account_list_paging(0));
    }
}
