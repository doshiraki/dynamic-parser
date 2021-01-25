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
        let ptn = "^(".to_string()+s.clone().as_str()+")";
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
    fn json_ok() {
        let json_boolean = Parser::regex("true", 0).or(Parser::regex("false", 0));
        let json_string = Parser::regex("\"([^\"]*)\"", 1);
        let json_number = Parser::regex("0|[1-9][0-9]*", 0);
        let json_item = json_boolean.clone().or(json_string.clone()).or(json_number.clone());

        let json_array = Parser::new(Box::new(move |root:&Parser|
                Parser::skip("\\[")
                .and(root.clone().and(Parser::skip(",?")).repeat())
                .and(Parser::skip("]"))
            ));

        let json_object = Parser::new(Box::new(move |root:&Parser|
                Parser::skip("\\{")
                .and(json_string.clone().and(Parser::skip(":")).and(root.clone()).and(Parser::skip(",?")).repeat())
                .and(Parser::skip("}"))
            ));
    
        let json_elements = json_item.clone()
                        .or(json_array.clone())
                        .or(json_object.clone());

        let result = json_elements.parse("{\"arr\":[123,456,789],\"obj\":{\"key\":\"value\",\"key\":123}}");
        assert_eq!(result.is_ok(), true);
        assert_eq!(
            result.value(),
            Value::List(vec![
                Value::List(vec![
                    Value::Some("arr".to_string()),
                    Value::List(vec![
                        Value::Some("123".to_string()),
                        Value::Some("456".to_string()),
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

