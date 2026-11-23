use junit_report::Duration as JUnitDuration;
use junit_report::{ReportBuilder, TestCaseBuilder, TestSuite, TestSuiteBuilder};
use quick_xml::events::{BytesText, Event};
use quick_xml::{Reader, Writer};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::time::Duration;

/// Collects the results of one connection as a JUnit test suite
#[derive(Debug)]
pub struct Reporter {
    test_suite: TestSuite,
}

impl Reporter {
    pub fn new(suite: &str) -> Reporter {
        Reporter {
            test_suite: TestSuiteBuilder::new(suite).build(),
        }
    }

    /// Creates a success test case in the current suite
    pub fn success(&mut self, name: &str, duration: Duration) {
        self.test_suite
            .add_testcase(TestCaseBuilder::success(name, to_junit(duration)).build());
    }

    /// Creates a failure test case in the current suite
    pub fn failure(&mut self, name: &str, duration: Duration, error_type: &str, message: &str) {
        self.test_suite.add_testcase(
            TestCaseBuilder::failure(name, to_junit(duration), error_type, message).build(),
        );
    }

    /// Creates an error test case in the current suite
    pub fn error(&mut self, name: &str, duration: Duration, error_type: &str, message: &str) {
        self.test_suite.add_testcase(
            TestCaseBuilder::error(name, to_junit(duration), error_type, message).build(),
        );
    }

    /// Creates a skipped test case in the current suite
    pub fn skipped(&mut self, name: &str) {
        self.test_suite
            .add_testcase(TestCaseBuilder::skipped(name).build());
    }

    pub fn into_suite(self) -> TestSuite {
        self.test_suite
    }
}

/// Writes all connection suites, in connection order, as one JUnit XML report.
/// With several connections each suite is suffixed with its connection number, so
/// they stay distinguishable; a single connection keeps the plain mapping name.
pub fn write_report(output_path: &str, mut suites: Vec<TestSuite>) -> crate::Result<()> {
    if suites.len() > 1 {
        for (index, suite) in suites.iter_mut().enumerate() {
            suite.name = format!("{} (connection {})", suite.name, index + 1);
            suite.package = format!("testsuite/{}", suite.name);
        }
    }

    let report = ReportBuilder::new().add_testsuites(suites).build();
    let mut compact = Vec::new();
    report.write_xml(&mut compact)?;

    let mut output = BufWriter::new(File::create(output_path)?);
    indent_xml(&compact, &mut output)?;
    output.flush()?;
    Ok(())
}

/// Re-emits compact XML with two-space indentation. `junit_report` only writes
/// compact output; text and CDATA are copied as-is, so their content is unchanged.
fn indent_xml(compact: &[u8], output: impl Write) -> crate::Result<()> {
    let mut reader = Reader::from_reader(compact);
    let mut writer = Writer::new_with_indent(output, b' ', 2);
    // The reader splits text around entity references (`&amp;`). Each run is written
    // back as one text event, otherwise the writer would indent inside the text.
    let mut text = Vec::new();

    loop {
        match reader.read_event()? {
            Event::Text(part) => text.extend_from_slice(&part),
            Event::GeneralRef(entity) => {
                text.push(b'&');
                text.extend_from_slice(&entity);
                text.push(b';');
            }
            event => {
                if !text.is_empty() {
                    let escaped = String::from_utf8(std::mem::take(&mut text))?;
                    writer.write_event(Event::Text(BytesText::from_escaped(escaped)))?;
                }
                if let Event::Eof = event {
                    break;
                }
                writer.write_event(event)?;
            }
        }
    }
    writer.get_mut().write_all(b"\n")?;
    Ok(())
}

fn to_junit(duration: Duration) -> JUnitDuration {
    JUnitDuration::milliseconds(duration.as_millis() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indent_xml_keeps_content_intact() {
        let compact = br#"<?xml version="1.0" encoding="utf-8"?><a><b x="1 &amp; 2"/><c>text &lt;here&gt;</c><d><![CDATA[ raw
 data ]]></d></a>"#;

        let mut indented = Vec::new();
        indent_xml(compact, &mut indented).unwrap();

        assert_eq!(
            String::from_utf8(indented).unwrap(),
            r#"<?xml version="1.0" encoding="utf-8"?>
<a>
  <b x="1 &amp; 2"/>
  <c>text &lt;here&gt;</c>
  <d><![CDATA[ raw
 data ]]></d>
</a>
"#
        );
    }
}
