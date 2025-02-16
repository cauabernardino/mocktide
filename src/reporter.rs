use junit_report::Duration as JUnitDuration;
use junit_report::{ReportBuilder, TestCaseBuilder, TestSuite, TestSuiteBuilder};
use std::fs::File;
use tokio::time::Duration;

/// Reports a workflow result in JUnit format
#[derive(Debug)]
pub struct Reporter {
    test_suite: TestSuite,
}

#[derive(Debug)]
pub enum TestResult {
    Success,
    Error,
    Failure,
}

impl Reporter {
    pub fn new(suite: &str) -> Reporter {
        Reporter {
            test_suite: TestSuiteBuilder::new(suite).build(),
        }
    }

    /// Creates a sucess test case in the current report
    pub fn sucess(&mut self, name: &str, duration: Duration) {
        let test_success =
            TestCaseBuilder::success(name, self.tokio_to_junit_duration(duration)).build();

        self.test_suite.add_testcase(test_success);
    }

    /// Creates a failure test case in the current report
    pub fn failure(&mut self, name: &str, duration: Duration, error_type: &str, message: &str) {
        let test_failure = TestCaseBuilder::failure(
            name,
            self.tokio_to_junit_duration(duration),
            error_type,
            message,
        )
        .build();

        self.test_suite.add_testcase(test_failure);
    }

    /// Creates a error test case in the current report
    pub fn error(&mut self, name: &str, duration: Duration, error_type: &str, message: &str) {
        let test_error = TestCaseBuilder::error(
            name,
            self.tokio_to_junit_duration(duration),
            error_type,
            message,
        )
        .build();

        self.test_suite.add_testcase(test_error);
    }

    /// Dump the final report in XML format in given `output_path`
    pub fn report(&mut self, output_path: &str) {
        let reporter = ReportBuilder::new()
            .add_testsuite(self.test_suite.clone())
            .build();

        let mut output_file = File::create(output_path).unwrap();
        reporter.write_xml(&mut output_file).unwrap();
    }

    /// Helper function to transfrom Duration from `tokio::time::Duration`
    /// to `junit_report::Duration`
    fn tokio_to_junit_duration(&self, duration: Duration) -> JUnitDuration {
        JUnitDuration::milliseconds(duration.as_millis() as i64)
    }
}
