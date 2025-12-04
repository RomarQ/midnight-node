use clap::Args;
use regex::Regex;
use xmltree::{Element, XMLNode};

// Marker format in <system-out>
// ### XRAY_TEST KEY:XRAY-111 STORIES:JIRA-123,JIRA-456 LABELS:automation,fast###
const XRAY_MARKER_REGEX: &str = r"###\s*XRAY_TEST\s+KEY:(?P<key>\S+)\s+STORIES:(?P<stories>[^#\r\n]*)\s+LABELS:(?P<labels>[^#\r\n]*)###";

// XML element names
const ELEM_TESTCASE: &str = "testcase";
const ELEM_SYSTEM_OUT: &str = "system-out";
const ELEM_PROPERTIES: &str = "properties";
const ELEM_PROPERTY: &str = "property";
const ATTR_NAME: &str = "name";
const ATTR_VALUE: &str = "value";

// Xray property names
const PROP_TEST_KEY: &str = "test_key";
const PROP_REQUIREMENTS: &str = "requirements";
const PROP_TAGS: &str = "tags";
const PROP_TEST_SUMMARY: &str = "test_summary";

#[derive(Args)]
pub struct EnrichJunitArgs {
	pub input: String,
	pub output: String,
}

pub fn enrich_junit(mut root: Element) -> anyhow::Result<Element, regex::Error> {
	let marker_re = Regex::new(XRAY_MARKER_REGEX)?;
	enrich_element(&mut root, &marker_re);
	Ok(root)
}

fn enrich_element(elem: &mut Element, marker_re: &regex::Regex) {
	if elem.name == ELEM_TESTCASE {
		enrich_testcase(elem, marker_re);
	}

	for child in elem.children.iter_mut() {
		if let XMLNode::Element(child_elem) = child {
			enrich_element(child_elem, marker_re);
		}
	}
}

fn enrich_testcase(testcase: &mut Element, marker_re: &regex::Regex) {
	// Find <system-out>
	let system_out_idx = testcase
		.children
		.iter()
		.position(|child| matches!(child, XMLNode::Element(e) if e.name == ELEM_SYSTEM_OUT));

	let system_out_idx = match system_out_idx {
		Some(idx) => idx,
		None => return,
	};

	let text = match &testcase.children[system_out_idx] {
		XMLNode::Element(e) => e.get_text().map(|t| t.to_string()),
		_ => None,
	};

	let text = match text {
		Some(t) => t,
		None => return,
	};

	let caps = match marker_re.captures(&text) {
		Some(c) => c,
		None => return,
	};

	let key = caps["key"].trim();
	let stories_raw = caps["stories"].trim();
	let labels = caps["labels"].trim();
	let name = testcase.attributes.get(ATTR_NAME).cloned().unwrap_or_default();

	let props_elem = get_or_create_properties_child(testcase);

	set_property(props_elem, PROP_TEST_KEY, key);
	set_property(props_elem, PROP_REQUIREMENTS, stories_raw);
	set_property(props_elem, PROP_TAGS, labels);
	set_property(props_elem, PROP_TEST_SUMMARY, &name);
}

fn get_or_create_properties_child(testcase: &mut Element) -> &mut Element {
	let existing_index = testcase
		.children
		.iter()
		.position(|child| matches!(child, XMLNode::Element(e) if e.name == ELEM_PROPERTIES));

	if let Some(idx) = existing_index {
		match testcase.children.get_mut(idx).unwrap() {
			XMLNode::Element(e) => return e,
			_ => unreachable!(),
		}
	}

	let props = Element::new(ELEM_PROPERTIES);
	testcase.children.insert(0, XMLNode::Element(props));

	match testcase.children.get_mut(0).unwrap() {
		XMLNode::Element(e) => e,
		_ => unreachable!(),
	}
}

fn set_property(props_elem: &mut Element, name: &str, value: &str) {
	let existing_index = props_elem.children.iter().position(|child| {
		matches!(child, XMLNode::Element(e)
            if e.name == ELEM_PROPERTY
                && e.attributes.get(ATTR_NAME).map(|v| v == name).unwrap_or(false))
	});

	if let Some(idx) = existing_index {
		if let XMLNode::Element(e) = props_elem.children.get_mut(idx).unwrap() {
			e.attributes.insert(ATTR_VALUE.to_string(), value.to_string());
		}
		return;
	}

	let mut prop = Element::new(ELEM_PROPERTY);
	prop.attributes.insert(ATTR_NAME.to_string(), name.to_string());
	prop.attributes.insert(ATTR_VALUE.to_string(), value.to_string());

	props_elem.children.push(XMLNode::Element(prop));
}

#[cfg(test)]
mod tests {
	use super::*;
	use xmltree::{Element, XMLNode};

	#[test]
	fn adds_xray_properties_from_system_out_single_case() {
		let input_xml = r#"
        <testsuite name="e2e">
          <testcase name="my_super_test">
            <system-out>
### XRAY_TEST KEY:XRAY-123 STORIES:JIRA-123,JIRA-124 LABELS:integration,fast ###
some other log...
            </system-out>
          </testcase>
        </testsuite>
        "#;

		let root = Element::parse(input_xml.as_bytes()).unwrap();
		let enriched = enrich_junit(root).unwrap();

		let testcase = enriched.get_child(ELEM_TESTCASE).expect("testcase not found");
		let properties = testcase.get_child(ELEM_PROPERTIES).expect("properties not inserted");

		let props = collect_properties(properties);
		assert_eq!(props.get(PROP_TEST_KEY).unwrap(), "XRAY-123");
		assert_eq!(props.get(PROP_REQUIREMENTS).unwrap(), "JIRA-123,JIRA-124");
		assert_eq!(props.get(PROP_TAGS).unwrap(), "integration,fast");
		assert_eq!(props.get(PROP_TEST_SUMMARY).unwrap(), "my_super_test");
	}

	#[test]
	fn handles_multiple_testcases_both_with_marker() {
		let input_xml = r#"
        <testsuite name="e2e">
          <testcase name="first_case">
            <system-out>
### XRAY_TEST KEY:XRAY-111 STORIES:JIRA-111,JIRA-112 LABELS:integration,fast ###
some other log...
            </system-out>
          </testcase>
          <testcase name="second_case">
            <system-out>
### XRAY_TEST KEY:XRAY-222 STORIES:JIRA-221,JIRA-222 LABELS:integration,slow ###
more logs...
            </system-out>
          </testcase>
        </testsuite>
        "#;

		let root = Element::parse(input_xml.as_bytes()).unwrap();
		let enriched = enrich_junit(root).unwrap();

		let testcases: Vec<_> = enriched
			.children
			.iter()
			.filter_map(|c| match c {
				XMLNode::Element(e) if e.name == ELEM_TESTCASE => Some(e),
				_ => None,
			})
			.collect();

		assert_eq!(testcases.len(), 2);

		let tc1 = &testcases[0];
		assert_eq!(tc1.attributes.get(ATTR_NAME).unwrap(), "first_case");
		let props1_elem = tc1
			.get_child(ELEM_PROPERTIES)
			.expect("properties not inserted for first testcase");
		let props1 = collect_properties(props1_elem);
		assert_eq!(props1.get(PROP_TEST_KEY).unwrap(), "XRAY-111");
		assert_eq!(props1.get(PROP_REQUIREMENTS).unwrap(), "JIRA-111,JIRA-112");
		assert_eq!(props1.get(PROP_TEST_SUMMARY).unwrap(), "first_case");
		assert_eq!(props1.get(PROP_TAGS).unwrap(), "integration,fast");

		let tc2 = &testcases[1];
		assert_eq!(tc2.attributes.get(ATTR_NAME).unwrap(), "second_case");
		let props2_elem = tc2
			.get_child(ELEM_PROPERTIES)
			.expect("properties not inserted for second testcase");
		let props2 = collect_properties(props2_elem);
		assert_eq!(props2.get(PROP_TEST_KEY).unwrap(), "XRAY-222");
		assert_eq!(props2.get(PROP_REQUIREMENTS).unwrap(), "JIRA-221,JIRA-222");
		assert_eq!(props2.get(PROP_TEST_SUMMARY).unwrap(), "second_case");
		assert_eq!(props2.get(PROP_TAGS).unwrap(), "integration,slow");
	}

	#[test]
	fn handles_multiple_testcases_only_enriching_those_with_marker() {
		let input_xml = r#"
        <testsuite name="e2e">
          <testcase name="with_marker">
            <system-out>
### XRAY_TEST KEY:XRAY-123 STORIES:JIRA-123,JIRA-124 LABELS:automation,fast ###
some other log...
            </system-out>
          </testcase>
          <testcase name="without_marker">
            <system-out>
just some logs, no xray marker here
            </system-out>
          </testcase>
        </testsuite>
        "#;

		let root = Element::parse(input_xml.as_bytes()).unwrap();
		let enriched = enrich_junit(root).unwrap();

		let testcases: Vec<_> = enriched
			.children
			.iter()
			.filter_map(|c| match c {
				XMLNode::Element(e) if e.name == ELEM_TESTCASE => Some(e),
				_ => None,
			})
			.collect();

		assert_eq!(testcases.len(), 2);

		let tc_with = &testcases[0];
		let tc_without = &testcases[1];

		let props_with = tc_with
			.get_child(ELEM_PROPERTIES)
			.map(collect_properties)
			.expect("properties should exist on testcase with marker");

		assert_eq!(props_with.get(PROP_TEST_KEY).unwrap(), "XRAY-123");
		assert_eq!(props_with.get(PROP_REQUIREMENTS).unwrap(), "JIRA-123,JIRA-124");
		assert_eq!(props_with.get(PROP_TEST_SUMMARY).unwrap(), "with_marker");
		assert_eq!(props_with.get(PROP_TAGS).unwrap(), "automation,fast");
		assert!(tc_without.get_child(ELEM_PROPERTIES).is_none());
	}

	#[test]
	fn handles_empty_stories_field() {
		let input_xml = r#"
        <testsuite name="e2e">
          <testcase name="no_stories">
            <system-out>
### XRAY_TEST KEY:XRAY-999 STORIES: LABELS: ###
some other log...
            </system-out>
          </testcase>
        </testsuite>
        "#;

		let root = Element::parse(input_xml.as_bytes()).unwrap();
		let enriched = enrich_junit(root).unwrap();

		let testcase = enriched.get_child(ELEM_TESTCASE).expect("testcase not found");
		let properties = testcase.get_child(ELEM_PROPERTIES).expect("properties not inserted");

		let props = collect_properties(properties);

		assert_eq!(props.get(PROP_TEST_KEY).unwrap(), "XRAY-999");
		assert!(props.contains_key(PROP_REQUIREMENTS));
		assert!(props.contains_key(PROP_TAGS));
		assert_eq!(props.get(PROP_REQUIREMENTS).unwrap(), "");
		assert_eq!(props.get(PROP_TEST_SUMMARY).unwrap(), "no_stories");
	}

	fn collect_properties(props_elem: &Element) -> std::collections::HashMap<String, String> {
		let mut map = std::collections::HashMap::new();

		for child in props_elem.children.iter() {
			if let XMLNode::Element(e) = child
				&& e.name == ELEM_PROPERTY
				&& let Some(name) = e.attributes.get(ATTR_NAME)
				&& let Some(value) = e.attributes.get(ATTR_VALUE)
			{
				map.insert(name.clone(), value.clone());
			}
		}

		map
	}
}
