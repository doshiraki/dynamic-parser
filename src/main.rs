use std::rc::Rc;
use regex::Regex;

#[derive(Debug, Clone, PartialEq)]
enum Value {
    None,
    Some(String),
    List(Vec<Value>),
}

#[derive(Debug)]
struct Success {
    pub position: i32,
    pub value: Value,
}

#[derive(Debug)]
struct Failure {
    pub position: i32,
    pub expected: Vec<String>,
}
trait Reply {
    fn position(&self) -> i32;
    fn err_position(&self) -> i32;
    fn value(&self) -> Value;
    fn expected(&self) -> Vec<String>;
}

impl Reply for Result<Success, Failure> {
    fn position(&self) -> i32 {
        match self {
            Ok(success) => success.position,
            Err(failure) => failure.position,
        }
    }

    fn err_position(&self) -> i32 {
        match self {
            Ok(_) => -1,
            Err(failure) => failure.position,
        }
    }

    fn value(&self) -> Value {
        match self {
            Ok(success) => success.value.clone(),
            Err(_) => panic!(),
        }
    }

    fn expected(&self) -> Vec<String> {
        match self {
            Ok(_) => panic!(),
            Err(failure) => failure.expected.to_vec(),
        }
    }
}

type ParserFunc = Rc<dyn Fn(&Parser, &str, i32) -> Result<Success, Failure>>;

#[derive(Clone)]
struct Parser
{
    pub func:ParserFunc,
}


impl<'b> Parser {
    fn new(p2p:Box<dyn Fn(&Parser) -> Parser>)->Self {
        Parser{func:Rc::new(move |root:&Parser, source: &str, position: i32|(p2p(root).func)(root, source, position))}
    }
    fn parse(&self, s:&str)->Result<Success, Failure> {
        let success = (self.func)(self, s, 0)?;
        if success.position < s.chars().count() as i32 {
            return Err(Failure{position: success.position, expected:vec!["no length".to_string()]});
        }
        Ok(success)
    }
    fn and(self, p:Self)->Self {
        Parser{func:Rc::new(move |root:&Self, s:&str, i:i32| {
            let result1 = (self.func)(root, s, i)?;
            let result2 = (p.func)(root, s, result1.position)?;
            let mut v = Vec::<Value>::new();
            if result1.value != Value::None {
                v.push(result1.value);
            }
            if result2.value != Value::None {
                v.push(result2.value);
            }
            Ok(Success{position: result2.position, value: 
                match v.len() {
                    0 => Value::None,
                    1 => v[0].clone(),
                    _ => Value::List(v),
                }})
        })}
    }

    fn list(self)->Self {
        Parser{func:Rc::new(move |root:&Self, s:&str, i:i32| {
            let mut result1 = (self.func)(root, s, i)?;
            if result1.value != Value::None {
                result1.value = Value::List(vec![result1.value]);
            }
            Ok(result1)
        })}
    }

    fn flat(self)->Self {
        Parser{func:Rc::new(move |root:&Self, s:&str, i:i32| {
            let mut result1 = (self.func)(root, s, i)?;
            if let Value::List(results) = result1.value {
                let mut v = Vec::<Value>::new();
                for result in results {
                    if let Value::List(result_each) = result {
                        v.extend(result_each)
                    } else if result != Value::None {
                        v.push(result)
                    }
                }
                result1.value = match v.len() {0=>Value::None, _=>Value::List(v)};
            };
            Ok(result1)
        })}
    }
    fn repeat(self)->Self {
        Parser{func:Rc::new(move |root:&Self, s:&str, pi:i32| {
            let mut v = Vec::<Value>::new();
            let mut i = pi;
            let pos = loop {
                let result = (self.func)(root, s, i);
                match result {
                    Err(_) => break i,
                    Ok(success) =>{
                        i = success.position;
                        if success.value != Value::None {
                            v.push(success.value);
                        }
                    }
                }
            };
            Ok(Success{position: pos, value: Value::List(v)})
        })}
    }

    fn merge_errs(e1:Failure, e2:Failure)-> Failure {
        let mut pos = e1.position;
        let mut e = Vec::<String>::new();
        if e1.position >= e2.position {
            e.extend(e1.expected.into_iter());
        }
        if e1.position <= e2.position {
            e.extend(e2.expected.into_iter());
            pos = e2.position;
        }
        Failure{position:pos, expected: e}
    }

    fn or(self, p:Self)->Self {
        Parser{func:Rc::new(move |root:&Self, s:&str, i:i32| {
            match (self.func)(root, s, i) {
                Err(e1) => 
                    match (p.func)(root, s, i){
                        Err(e2) => Err(Parser::merge_errs(e1, e2)),
                        ok => ok,
                    },
                ok => ok,
            }
        })}
    }

    fn skip(pattern: &str) -> Self {
        Parser::regex(pattern, -1)
    }

    fn regex(pattern: &str, group: isize) -> Self {
        let s = pattern.to_string();
        let ptn = "^(".to_string()+s.as_str()+")";
        let regex = Regex::new(&ptn).unwrap();
        Parser{func:Rc::new(move |_root:&Self, source: &str, position: i32| -> Result<Success, Failure> {
            let src = &source[position as usize..source.len()];
            let captures = regex.captures(src);
            match captures {
                Some(caps) => {
                    let text = if group < 0 {""}else{caps.get(group as usize + 1).unwrap().as_str()};
                    let mat = caps.get(0).unwrap();
                    Ok(Success {
                        position: position + (mat.end() - mat.start()) as i32,
                        value: if group < 0 {Value::None}else{Value::Some(text.to_string())},
                    })
                }
                None => Err(Failure {
                    position: position,
                    expected: vec![s.clone()],
                })
            }
        })}
    }
}



fn main() {
    println!("Hello, world!");
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn and_ok() {
        let string = |p:&str| Parser::regex(p, 0);
        let parser = string("key").and(string(":")).and(string("value"));
        let result = parser.parse("key:value");
        assert_eq!(result.is_ok(), true);
        assert_eq!(
            result.value(),
            Value::List(vec![
                Value::List(vec![
                    Value::Some("key".to_string()),
                    Value::Some(":".to_string()),
                ]),
                Value::Some("value".to_string()),
            ]),
        );
    }

    #[test]
    fn and_error() {
        let string = |p:&str| Parser::regex(p, 0);
        let parser = string("key").and(string(":")).and(string("value"));
        let result = parser.parse("key:valu");
        assert_eq!(result.is_ok(), false);
        assert_eq!(result.err_position(), 4);
    }

    #[test]
    fn or_ok() {
        let string = |p:&str| Parser::regex(p, 0);
        let parser = string("x").or(string("y")).or(string("z"));
        let result = parser.parse("x");
        assert_eq!(result.is_ok(), true);
        assert_eq!(result.value(), Value::Some("x".to_string()));
    }

    #[test]
    fn or_error() {
        let string = |p:&str| Parser::regex(p, 0);
        let parser = string("x").or(string("y")).or(string("z"));
        let result = parser.parse("w");
        assert_eq!(result.is_ok(), false);
        assert_eq!(result.err_position(), 0);
    }

    #[test]
    fn many_ok() {
        let string = |p:&str| Parser::regex(p, 0);
        let parser = string("xy").repeat().flat();
        let result = parser.parse("xyxyxyxy");
        assert_eq!(result.is_ok(), true);
        assert_eq!(
            result.value(),
            Value::List(vec![
                Value::Some("xy".to_string()),
                Value::Some("xy".to_string()),
                Value::Some("xy".to_string()),
                Value::Some("xy".to_string()),
            ]),
        );

        let parser = string("xy").repeat().flat();
        let result = parser.parse("");
        assert_eq!(result.is_ok(), true);
        assert_eq!(
            result.value(),
            Value::None,
        );
    }

    #[test]
    fn many_error() {
        let string = |p:&str| Parser::regex(p, 0);
        let parser = string("x").repeat();
        let result = parser.parse("xxxxxy");
        assert_eq!(result.is_ok(), false);
        assert_eq!(result.err_position(), 5);
    }

    #[test]
    fn regex_ok() {
        let parser = Parser::regex(r"([0-9]+)([a-z]+)", 1);
        let result = parser.parse("123abc");
        assert_eq!(result.is_ok(), true);
        assert_eq!(result.value(), Value::Some("123".to_string()));

        let parser = Parser::regex(r"[0-9]+", 0);
        let result = parser.parse("123");
        assert_eq!(result.is_ok(), true);
        assert_eq!(result.value(), Value::Some("123".to_string()));
    }

    #[test]
    fn regex_error() {
        let parser = Parser::regex(r"[0-9]+", 0);
        let result = parser.parse("12a");
        assert_eq!(result.is_ok(), false);
        assert_eq!(result.err_position(), 2);
    }

    #[test]
    fn sep_by1_ok() {
        let string = |p:&str| Parser::regex(p, 0);
        let parser_val = string("val");
        let parser = parser_val.clone().and(Parser::skip(",").and(parser_val.clone()).repeat()).flat();

        let result = parser.parse("val");
        assert_eq!(result.is_ok(), true);
        assert_eq!(
            result.value(),
            Value::List(vec![
                Value::Some("val".to_string()),
            ]),
        );

        let result = parser.parse("val,val,val");
        assert_eq!(result.is_ok(), true);
        assert_eq!(
            result.value(),
            Value::List(vec![
                Value::Some("val".to_string()),
                Value::Some("val".to_string()),
                Value::Some("val".to_string()),
            ]),
        );
    }

    #[test]
    fn sep_by1_error() {
        let string = |p:&str| Parser::regex(p, 0);
        let parser_val = string("val");
        let parser = parser_val.clone().and(Parser::skip(",").and(parser_val.clone()).repeat()).flat();

        let result = parser.parse("");
        assert_eq!(result.is_ok(), false);
        assert_eq!(result.err_position(), 0);

        let result = parser.parse("val,");
        assert_eq!(result.is_ok(), false);
        assert_eq!(result.err_position(), 3);
    }

    #[test]
    fn sep_by_ok() {
        let string = |p:&str| Parser::regex(p, 0);
        let parser_val = string("val");
        let parser = parser_val.clone().and(Parser::skip(",").and(string("val")).repeat()).flat().or(Parser::skip(""));

        let result = parser.parse("");
        assert_eq!(result.is_ok(), true);
        assert_eq!(
            result.value(),
            Value::None,
        );

        let result = parser.parse("val");
        assert_eq!(result.is_ok(), true);
        assert_eq!(
            result.value(),
            Value::List(vec![
                Value::Some("val".to_string()),
            ]),
        );

        let result = parser.parse("val,val,val");
        assert_eq!(result.is_ok(), true);
        assert_eq!(
            result.value(),
            Value::List(vec![
                Value::Some("val".to_string()),
                Value::Some("val".to_string()),
                Value::Some("val".to_string()),
            ]),
        );
    }

    #[test]
    fn sep_by_error() {
        let string = |p:&str| Parser::regex(p, 0);
        let parser_val = string("val");
        let parser = parser_val.clone().and(Parser::skip(",").and(string("val")).repeat()).flat().or(Parser::skip(""));
        let result = parser.parse("val,");
        assert_eq!(result.is_ok(), false);
        assert_eq!(result.err_position(), 3);
    }

    #[test]
    fn skip_ok() {
        let parser = Parser::regex("x", 0).and(Parser::skip("y"));
        let result = parser.parse("xy");
        assert_eq!(result.is_ok(), true);
        assert_eq!(result.value(), Value::Some("x".to_string()));
    }

    #[test]
    fn skip_error() {
        let parser = Parser::regex("xxx", 0).and(Parser::skip("yyy"));
        let result = parser.parse("xxxxyy");
        assert_eq!(result.is_ok(), false);
        assert_eq!(result.err_position(), 3);
    }

    #[test]
    fn string_ok() {
        let string = |p:&str| Parser::regex(p, 0);
        let parser = string("source");
        let result = parser.parse("source");
        assert_eq!(result.is_ok(), true);
        assert_eq!(result.value(), Value::Some("source".to_string()));
    }

    #[test]
    fn string_error() {
        let string = |p:&str| Parser::regex(p, 0);
        let parser = string("source");
        let result = parser.parse("other");
        assert_eq!(result.is_ok(), false);
        assert_eq!(result.err_position(), 0);
    }

    #[test]
    fn then_ok() {
        let parser = Parser::skip("x").and(Parser::regex("y", 0));
        let result = parser.parse("xy");
        assert_eq!(result.is_ok(), true);
        assert_eq!(result.value(), Value::Some("y".to_string()));
    }

    #[test]
    fn then_error() {
        let parser = Parser::skip("xxx").and(Parser::regex("yyy", 0));
        let result = parser.parse("xxxxyy");
        assert_eq!(result.is_ok(), false);
        assert_eq!(result.err_position(), 3);
    }


    #[test]
    fn json_ok() {
        let json_boolean = Parser::regex("true", 0).or(Parser::regex("false", 0));
        let json_quot = Parser::skip("\"");
        let json_string = json_quot.clone().and(Parser::regex("([^\\\\\"]*(\\\\.)?)+", 0)).and(json_quot.clone());
        let json_number = Parser::regex("-?(0|[1-9][0-9]*)", 0);
        let json_item = json_boolean.clone().or(json_string.clone()).or(json_number.clone());

        let json_array = Parser::new(Box::new(move |root:&Parser|
                Parser::skip("\\[")
                .and(root.clone().and(Parser::skip(",")).repeat().and(root.clone().or(Parser::skip(""))).flat())
                .and(Parser::skip("]"))
            ));

        let json_string_for_object = json_string.clone();
        let json_object = Parser::new(Box::new(move |root:&Parser|{
            let json_pair = json_string_for_object.clone().and(Parser::skip(":")).and(root.clone());
            let json_comma = Parser::skip(",");
            Parser::skip("\\{")
            .and(
                json_pair.clone().list()
                .and(json_comma.clone().and(json_pair.clone()).repeat()).flat()
                .and(json_comma.clone().or(Parser::skip(""))))
            .and(Parser::skip("}"))
            }));
    
        let json_elements = json_item.clone()
                        .or(json_array.clone())
                        .or(json_object.clone());


        let result = json_boolean.parse("true");
        assert_eq!(result.is_ok(), true);
        assert_eq!(result.value(), Value::Some("true".to_string()));

        let result = json_boolean.parse("false");
        assert_eq!(result.is_ok(), true);
        assert_eq!(result.value(), Value::Some("false".to_string()));
                
        let result = json_number.parse("-123");
        assert_eq!(result.is_ok(), true);
        assert_eq!(result.value(), Value::Some("-123".to_string()));

        let result = json_number.parse("1230");
        assert_eq!(result.is_ok(), true);
        assert_eq!(result.value(), Value::Some("1230".to_string()));

        let result = json_string.parse("\"foobar\"");
        assert_eq!(result.is_ok(), true);
        assert_eq!(result.value(), Value::Some("foobar".to_string()));

        let result = json_string.parse("\"\"");
        assert_eq!(result.is_ok(), true);
        assert_eq!(result.value(), Value::Some("".to_string()));

        let result = json_elements.parse("[\"foo\",\"bar\"]");
        assert_eq!(result.is_ok(), true);
        assert_eq!(
            result.value(),
            Value::List(vec![
                Value::Some("foo".to_string()),
                Value::Some("bar".to_string()),
            ]),
        );

        let result = json_elements.parse("[]");
        assert_eq!(result.is_ok(), true);
        assert_eq!(
            result.value(),
            Value::None,
        );

        let result = json_elements.parse("[,]");
        assert_eq!(result.is_ok(), false);
        assert_eq!(result.err_position(), 1);

        let result = json_elements.parse("[123,456,]");
        assert_eq!(result.is_ok(), true);
        assert_eq!(
            result.value(),
            Value::List(vec![
                Value::Some("123".to_string()),
                Value::Some("456".to_string()),
            ]),
        );

        let result = json_elements.parse("[123,456,789]");
        assert_eq!(result.is_ok(), true);
        assert_eq!(
            result.value(),
            Value::List(vec![
                Value::Some("123".to_string()),
                Value::Some("456".to_string()),
                Value::Some("789".to_string()),
            ]),
        );

        let result = json_elements.parse("[123\"456\"]");
        assert_eq!(result.is_ok(), false);
        assert_eq!(result.err_position(),4);

        let result = json_elements.parse("{\"key1\":\"value\",\"key2\":123,}");
        assert_eq!(result.is_ok(), true);
        assert_eq!(
            result.value(),
            Value::List(vec![
                Value::List(vec![
                    Value::Some("key1".to_string()),
                    Value::Some("value".to_string()),
                ]),
                Value::List(vec![
                    Value::Some("key2".to_string()),
                    Value::Some("123".to_string()),
                ]),
            ]),
        );

        let result = json_elements.parse("{\"key1\":\"value\",\"key2\":123,\"key3\":true,}");
        assert_eq!(result.is_ok(), true);
        assert_eq!(
            result.value(),
            Value::List(vec![
                Value::List(vec![
                    Value::Some("key1".to_string()),
                    Value::Some("value".to_string()),
                ]),
                Value::List(vec![
                    Value::Some("key2".to_string()),
                    Value::Some("123".to_string()),
                ]),
                Value::List(vec![
                    Value::Some("key3".to_string()),
                    Value::Some("true".to_string()),
                ]),
            ]),
        );

        let result = json_elements.parse("{}");
        assert_eq!(result.is_ok(), false);
        assert_eq!(result.err_position(), 1);

        let result = json_elements.parse("{,}");
        assert_eq!(result.is_ok(), false);
        assert_eq!(result.err_position(), 1);

        let result = json_elements.parse("{\"arr\":[123,\"4\\\"56\",789],\"obj\":{\"key\":\"value\",\"key\":123},}");
        assert_eq!(result.is_ok(), true);
        assert_eq!(
            result.value(),
            Value::List(vec![
                Value::List(vec![
                    Value::Some("arr".to_string()),
                    Value::List(vec![
                        Value::Some("123".to_string()),
                        Value::Some("4\\\"56".to_string()),
                        Value::Some("789".to_string()),
                    ]),
                ]),
                Value::List(vec![
                    Value::Some("obj".to_string()),
                    Value::List(vec![
                        Value::List(vec![
                            Value::Some("key".to_string()),
                            Value::Some("value".to_string()),
                        ]),
                        Value::List(vec![
                            Value::Some("key".to_string()),
                            Value::Some("123".to_string()),
                        ]),
                    ]),
                ]),
            ]),
        );
      

    }
}

