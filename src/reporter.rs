use junit_report::Duration as JUnitDuration;
use junit_report::{ReportBuilder, TestCaseBuilder, TestSuite, TestSuiteBuilder};
use std::fs::File;
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
    report.write_xml(File::create(output_path)?)?;
    Ok(())
}

fn to_junit(duration: Duration) -> JUnitDuration {
    JUnitDuration::milliseconds(duration.as_millis() as i64)
}
