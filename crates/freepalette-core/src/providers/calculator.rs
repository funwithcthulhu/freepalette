use freepalette_plugin_api::{
    Action, ActionOutcome, PluginError, Provider, ProviderId, ResultKind, SearchContext,
    SearchResult,
};
use thiserror::Error;

const CALCULATOR_SCORE_HINT: i64 = 1_000;

pub struct CalculatorProvider;

impl Provider for CalculatorProvider {
    fn id(&self) -> ProviderId {
        ProviderId::from("calculator")
    }

    fn search(&self, context: &SearchContext) -> Result<Vec<SearchResult>, PluginError> {
        let Some(expression) = expression_from_query(context.query.raw()) else {
            return Ok(Vec::new());
        };

        let value = evaluate_expression(expression)
            .map_err(|source| PluginError::InvalidQuery(source.to_string()))?;
        let formatted_value = format_number(value);

        Ok(vec![SearchResult::new(
            self.id(),
            format!("calc:{expression}"),
            format!("{} = {}", expression.trim(), formatted_value),
            ResultKind::Calculator,
            Action::CopyText {
                text: formatted_value.clone(),
            },
        )
        .with_subtitle("Calculator")
        .with_keywords(vec!["calc".to_string(), "calculator".to_string()])
        .with_score_hint(CALCULATOR_SCORE_HINT)])
    }

    fn execute(&self, action: &Action) -> Result<ActionOutcome, PluginError> {
        match action {
            Action::CopyText { text } => Ok(ActionOutcome::new(format!(
                "calculator result ready to copy: {text}"
            ))),
            _ => Err(PluginError::UnsupportedAction),
        }
    }
}

fn expression_from_query(query: &str) -> Option<&str> {
    query
        .trim()
        .strip_prefix("calc ")
        .map(str::trim)
        .filter(|expression| !expression.is_empty())
}

fn evaluate_expression(expression: &str) -> Result<f64, CalculatorError> {
    let mut parser = Parser::new(expression);
    let value = parser.parse_expression()?;
    parser.skip_whitespace();
    if parser.is_at_end() {
        Ok(value)
    } else {
        Err(CalculatorError::UnexpectedToken)
    }
}

fn format_number(value: f64) -> String {
    if !value.is_finite() {
        return value.to_string();
    }

    if value.fract().abs() < f64::EPSILON {
        return format!("{value:.0}");
    }

    let mut output = format!("{value:.10}");
    while output.ends_with('0') {
        output.pop();
    }
    if output.ends_with('.') {
        output.pop();
    }
    output
}

#[derive(Debug, Error, PartialEq)]
enum CalculatorError {
    #[error("division by zero")]
    DivisionByZero,
    #[error("expected a number")]
    ExpectedNumber,
    #[error("unclosed parenthesis")]
    UnclosedParenthesis,
    #[error("unexpected token")]
    UnexpectedToken,
}

struct Parser<'a> {
    input: &'a [u8],
    position: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
            position: 0,
        }
    }

    fn parse_expression(&mut self) -> Result<f64, CalculatorError> {
        let mut value = self.parse_term()?;

        loop {
            self.skip_whitespace();
            if self.consume(b'+') {
                value += self.parse_term()?;
            } else if self.consume(b'-') {
                value -= self.parse_term()?;
            } else {
                return Ok(value);
            }
        }
    }

    fn parse_term(&mut self) -> Result<f64, CalculatorError> {
        let mut value = self.parse_factor()?;

        loop {
            self.skip_whitespace();
            if self.consume(b'*') {
                value *= self.parse_factor()?;
            } else if self.consume(b'/') {
                let divisor = self.parse_factor()?;
                if divisor.abs() < f64::EPSILON {
                    return Err(CalculatorError::DivisionByZero);
                }
                value /= divisor;
            } else {
                return Ok(value);
            }
        }
    }

    fn parse_factor(&mut self) -> Result<f64, CalculatorError> {
        self.skip_whitespace();

        if self.consume(b'+') {
            return self.parse_factor();
        }
        if self.consume(b'-') {
            return Ok(-self.parse_factor()?);
        }
        if self.consume(b'(') {
            let value = self.parse_expression()?;
            self.skip_whitespace();
            if self.consume(b')') {
                return Ok(value);
            }
            return Err(CalculatorError::UnclosedParenthesis);
        }

        self.parse_number()
    }

    fn parse_number(&mut self) -> Result<f64, CalculatorError> {
        self.skip_whitespace();
        let start = self.position;
        let mut dot_count = 0;

        while let Some(byte) = self.peek() {
            if byte.is_ascii_digit() {
                self.position += 1;
            } else if byte == b'.' && dot_count == 0 {
                dot_count += 1;
                self.position += 1;
            } else {
                break;
            }
        }

        if start == self.position {
            return Err(CalculatorError::ExpectedNumber);
        }

        let token = std::str::from_utf8(&self.input[start..self.position])
            .map_err(|_| CalculatorError::ExpectedNumber)?;
        token
            .parse::<f64>()
            .map_err(|_| CalculatorError::ExpectedNumber)
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(byte) if byte.is_ascii_whitespace()) {
            self.position += 1;
        }
    }

    fn consume(&mut self, expected: u8) -> bool {
        if self.peek() == Some(expected) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.position).copied()
    }

    fn is_at_end(&self) -> bool {
        self.position >= self.input.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_basic_math() {
        assert_eq!(
            evaluate_expression("2+2").expect("expression should parse"),
            4.0
        );
        assert_eq!(
            evaluate_expression("2 * (3 + 4)").expect("expression should parse"),
            14.0
        );
        assert_eq!(
            evaluate_expression("-5 + 2").expect("expression should parse"),
            -3.0
        );
    }

    #[test]
    fn rejects_division_by_zero() {
        let error = evaluate_expression("10 / 0").expect_err("division by zero should fail");
        assert_eq!(error, CalculatorError::DivisionByZero);
    }

    #[test]
    fn detects_only_calc_prefix() {
        assert_eq!(expression_from_query("calc 2+2"), Some("2+2"));
        assert_eq!(expression_from_query("calc"), None);
        assert_eq!(expression_from_query("calc "), None);
        assert_eq!(expression_from_query("2+2"), None);
        assert_eq!(expression_from_query("=2+2"), None);
    }

    #[test]
    fn provider_returns_result_for_calc_query() {
        let provider = CalculatorProvider;
        let results = provider
            .search(&SearchContext::new("calc 2+2", 10))
            .expect("calculator search should succeed");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "2+2 = 4");
    }
}
