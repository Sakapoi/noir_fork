use std::{
    fs::File,
    io::{BufRead, BufReader, Read},
};

use askama::Template;
use noirc_frontend::{
    hir::resolution::errors::Span,
    lexer::Lexer,
    token::{DocStyle, Keyword, SpannedToken, Token},
};

use crate::{Function, Output};

/// Retrieves the content of a module from a list of tokens and the starting index.

/// The `get_module_content` function is designed to obtain the content of a module based on
/// a list of `tokens` and the starting index `index`. It looks for a module description that
/// begins at the index `index` and may be terminated by either semicolons `;` or curly braces `{}`.
/// If the module is terminated by semicolons `;`, the function loads the module content from a file
/// whose name is specified before the semicolon in the list of tokens.
pub(crate) fn get_module_content(
    tokens: &[Token],
    index: usize,
) -> Result<Vec<Output>, crate::DocError> {
    let mut content = Vec::new();
    let mut i = index;
    let mut brace_counter = 0;

    loop {
        match &tokens[i] {
            Token::Semicolon => {
                let filename = format!("input_files/{}.nr", tokens[i - 1]);
                content = get_doc(&filename).map_err(|_| crate::DocError::GetDocError)?;
                break;
            }
            Token::LeftBrace => {
                brace_counter += 1;
                i += 1;
                while brace_counter != 0 {
                    match &tokens[i] {
                        Token::LeftBrace => {
                            brace_counter += 1;
                            content
                                .push(SpannedToken::new(tokens[i].clone(), Span::inclusive(0, 1)));
                            i += 1;
                        }
                        Token::RightBrace => {
                            brace_counter -= 1;
                            content
                                .push(SpannedToken::new(tokens[i].clone(), Span::inclusive(0, 1)));
                            i += 1;
                        }
                        _ => {
                            content
                                .push(SpannedToken::new(tokens[i].clone(), Span::inclusive(0, 1)));
                            i += 1;
                        }
                    }
                }
                break;
            }
            _ => {
                i += 1;
            }
        };
    }

    let res = Output::to_output(content)?;

    Ok(res)
}

/// Skips an implementation block within a list of tokens, starting at the given index.

/// The `skip_impl_block` function is used to skip an implementation block within a list of tokens.
/// It starts at the specified index and continues until it finds the closing curly brace `}` of
/// the implementation block. This function is useful when you want to ignore or bypass an
/// implementation block in the token list.
pub(crate) fn skip_impl_block(tokens: &[Token], index: usize) -> usize {
    let mut brace_counter = 0;
    let mut i = index;

    while brace_counter != 1 {
        match &tokens[i] {
            Token::LeftBrace => {
                i += 1;
                brace_counter += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    while brace_counter != 0 {
        match &tokens[i] {
            Token::LeftBrace => {
                i += 1;
                brace_counter += 1;
            }
            Token::RightBrace => {
                i += 1;
                brace_counter -= 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    i - index - 1
}

pub(crate) fn fn_signature(tokens: &[Token], index: usize) -> String {
    let mut res = String::new();
    let mut i = index;
    loop {
        match &tokens[i] {
            Token::LeftBrace | Token::Semicolon => {
                break;
            }
            _ => {
                res.push_str(&tokens[i].to_string());
                res.push(' ');
                i += 1;
            }
        };
    }
    res
}

/// Extracts the function signature from a list of tokens starting at the given index.

/// The `fn_signature` function is designed to extract the function signature from a list of tokens.
/// It starts at the specified index and continues until it encounters a left curly brace `{` or a semicolon `;`,
/// indicating the start of the function's body or the end of the signature. The function returns the extracted
/// function signature as a string.
pub(crate) fn struct_signature(tokens: &[Token], index: usize) -> String {
    let mut res = String::new();
    let mut i = index;
    let mut is_private = true;

    loop {
        match &tokens[i] {
            Token::LeftBrace => {
                res.push('{');
                res.push('\n');
                loop {
                    match tokens[i] {
                        Token::RightBrace => {
                            if is_private {
                                res.push_str("/* private fields */");
                            }
                            res.push('\n');
                            res.push('}');
                            break;
                        }
                        Token::Keyword(Keyword::Pub) => {
                            is_private = false;
                            loop {
                                match tokens[i] {
                                    Token::Comma => {
                                        if tokens[i + 1] == Token::RightBrace {
                                            res.push(',');
                                        } else {
                                            res.push_str(",\n");
                                        }
                                        i += 1;
                                        break;
                                    }
                                    Token::RightBrace => {
                                        break;
                                    }
                                    _ => {
                                        res.push_str(&tokens[i].to_string());
                                        res.push(' ');
                                        i += 1;
                                    }
                                }
                            }
                        }
                        _ => {
                            i += 1;
                        }
                    }
                }
                break;
            }
            _ => {
                res.push_str(&tokens[i].to_string());
                res.push(' ');
                i += 1;
            }
        };
    }

    res
}

/// Extracts information about a trait from a list of tokens starting at the given index.

/// The `trait_info` function is used to extract information about a trait from a list of tokens.
/// It starts at the specified index and continues until it collects details about the trait's signature,
/// required methods, and provided methods. The extracted information is returned as a tuple, including
/// the trait's signature as a string, a vector of required methods, and a vector of provided methods.
pub(crate) fn trait_info(tokens: &[Token], index: usize) -> (String, Vec<Function>, Vec<Function>) {
    let mut sign = String::new();
    let mut required_methods = Vec::new();
    let mut provided_methods = Vec::new();
    let mut i = index;
    let mut brace_counter;

    loop {
        match &tokens[i + 1] {
            Token::LeftBrace => {
                sign.push('{');
                sign.push('\n');
                loop {
                    match tokens[i + 1] {
                        Token::RightBrace => {
                            sign.push('}');
                            break;
                        }
                        Token::Keyword(Keyword::Fn) => {
                            let name = match &tokens[i + 2] {
                                Token::Ident(idn) => idn.clone(),
                                _ => {
                                    break;
                                }
                            };
                            let doc = doc(tokens, i + 1);
                            let fn_sign = fn_signature(tokens, i + 1);

                            loop {
                                match tokens[i + 1] {
                                    Token::Semicolon => {
                                        required_methods.push(Function {
                                            name,
                                            doc,
                                            signature: fn_sign,
                                            is_method: true,
                                        });
                                        sign.push(';');
                                        sign.push('\n');
                                        break;
                                    }
                                    Token::LeftBrace => {
                                        provided_methods.push(Function {
                                            name,
                                            doc,
                                            signature: fn_sign,
                                            is_method: true,
                                        });
                                        brace_counter = 1;
                                        sign.push_str("{ ... }");
                                        sign.push('\n');
                                        while brace_counter != 0 {
                                            i += 1;
                                            match tokens[i + 1] {
                                                Token::LeftBrace => {
                                                    brace_counter += 1;
                                                }
                                                Token::RightBrace => {
                                                    brace_counter -= 1;
                                                }
                                                _ => {}
                                            }
                                        }
                                        i += 1;
                                        break;
                                    }
                                    _ => {
                                        sign.push_str(&tokens[i + 1].to_string());
                                        sign.push(' ');
                                        i += 1;
                                    }
                                }
                            }
                        }
                        _ => {
                            i += 1;
                        }
                    }
                }
                break;
            }
            _ => {
                sign.push_str(&tokens[i + 1].to_string());
                sign.push(' ');
                i += 1;
            }
        };
    }

    (sign, required_methods, provided_methods)
}

/// Extracts additional documentation preceding a code element from a list of tokens.

/// The `additional_doc` function is used to extract any additional documentation comments that
/// appear immediately before a code element in a list of tokens. These comments are often used to
/// provide context or explanations for the code that follows. The function starts at the specified
/// index and searches for any documentation comments that precede the code element, and then returns
/// the combined documentation as a string.
pub(crate) fn additional_doc(tokens: &[Token], index: usize) -> String {
    if index == 0 {
        return "".to_string();
    }
    match &tokens[index - 1] {
        Token::LineComment(dc, Some(DocStyle::Inner))
        | Token::BlockComment(dc, Some(DocStyle::Inner)) => {
            let mut res = dc.to_string();
            let mut doc_end = true;
            let mut iter = 2;
            while doc_end && ((index as i32) - (iter as i32)) >= 0 {
                match &tokens[index - iter] {
                    Token::LineComment(doc, Some(DocStyle::Inner))
                    | Token::BlockComment(doc, Some(DocStyle::Inner)) => {
                        res.insert_str(0, &doc.to_string());
                        iter += 1;
                    }
                    _ => {
                        doc_end = false;
                    }
                }
            }
            res
        }
        _ => {
            let mut res = String::new();

            let mut doc_find = true;
            let mut iter = 2;
            while doc_find && ((index as i32) - (iter as i32)) >= 0 {
                match &tokens[index - iter] {
                    Token::LineComment(doc, Some(DocStyle::Inner)) => {
                        res.insert_str(0, &doc.to_string());
                        iter += 1;
                    }
                    Token::Keyword(Keyword::Fn)
                    | Token::Keyword(Keyword::Mod)
                    | Token::Keyword(Keyword::Struct)
                    | Token::Keyword(Keyword::Trait)
                    | Token::Keyword(Keyword::Impl) => {
                        doc_find = false;
                    }
                    _ => {
                        iter += 1;
                    }
                }
            }
            res
        }
    }
}

/// Extracts documentation comments for a code element from a list of tokens.

/// The `doc` function is used to extract documentation comments associated with a code element
/// from a list of tokens. These comments are often used to provide explanations, descriptions,
/// or comments about the code element. The function starts at the specified index and searches
/// for relevant documentation comments, and then returns the combined documentation as a string.
pub(crate) fn doc(tokens: &[Token], index: usize) -> String {
    if index == 0 {
        return String::new();
    }
    match &tokens[index - 1] {
        Token::LineComment(dc, _) | Token::BlockComment(dc, _) => {
            let mut res = dc.to_string();
            let mut doc_end = true;
            let mut iter = 2;
            while doc_end && ((index as i32) - (iter as i32)) >= 0 {
                match &tokens[index - iter] {
                    Token::LineComment(doc, None) | Token::BlockComment(doc, None) => {
                        res.insert_str(0, &doc.to_string());
                        iter += 1;
                    }
                    _ => {
                        doc_end = false;
                    }
                }
            }
            res
        }
        _ => {
            let mut res = String::new();

            let mut doc_find = true;
            let mut iter = 2;
            while doc_find && ((index as i32) - (iter as i32)) >= 0 {
                match &tokens[index - iter] {
                    Token::LineComment(doc, Some(DocStyle::Outer))
                    | Token::BlockComment(doc, Some(DocStyle::Outer)) => {
                        res.insert_str(0, &doc.to_string());
                        iter += 1;
                    }
                    Token::Keyword(Keyword::Fn)
                    | Token::Keyword(Keyword::Mod)
                    | Token::Keyword(Keyword::Struct)
                    | Token::Keyword(Keyword::Trait)
                    | Token::Keyword(Keyword::Impl) => {
                        doc_find = false;
                    }
                    _ => {
                        iter += 1;
                    }
                }
            }
            res
        }
    }
}

/// Extracts an outer documentation comment associated with a code element from a list of tokens.

/// The `outer_doc` function is used to extract an outer documentation comment associated with a
/// code element from a list of tokens. Outer documentation comments are often used to provide
/// high-level explanations or descriptions for the code element. The function starts at the
/// specified index and searches for an outer documentation comment, returning the comment as a string
/// along with the updated index.
pub(crate) fn outer_doc(tokens: &[Token], index: usize) -> (String, usize) {
    let mut i = index;
    let mut res = tokens[i].to_string();
    let mut doc_find = true;
    while doc_find {
        match &tokens[i + 1] {
            Token::LineComment(doc, Some(DocStyle::Inner))
            | Token::BlockComment(doc, Some(DocStyle::Inner)) => {
                res.push_str(doc);
                i += 1;
            }
            _ => {
                doc_find = false;
            }
        }
    }

    if let Some(pos) = res.find(' ') {
        res = res.split_off(pos + 1);
    } else {
        res.clear();
    }

    (res, i)
}

/// Reads and tokenizes the content of a source file, returning a vector of spanned tokens.

/// The `get_doc` function reads the content of a source file specified by the `input_file` path,
/// tokenizes the content, and returns the resulting vector of spanned tokens. This function is
/// typically used for processing source code and extracting tokens for further analysis or documentation.
pub(crate) fn get_doc(input_file: &str) -> Result<Vec<SpannedToken>, crate::DocError> {
    let mut file = File::open(input_file).map_err(|_| crate::DocError::FileEditError)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).map_err(|_| crate::DocError::FileEditError)?;

    let token = Lexer::new(&contents)
        .skip_comments(false)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| crate::DocError::GetTokensError)?;

    Ok(token)
}

/// Represents a code block with associated code lines.

/// The `Code` struct represents a code block and is typically used to group a collection of
/// code lines. It is used in conjunction with the `CodeLine` struct to create code blocks for
/// documentation purposes.
#[derive(Template)]
#[template(path = "code_template.html")]
pub(crate) struct Code {
    pub(crate) codelines: Vec<CodeLine>,
}

/// Represents an individual line of code within a code block.

/// The `CodeLine` struct represents an individual line of code within a code block. It is typically
/// used within a `Code` structure to create a collection of code lines for documentation or rendering purposes.
#[derive(Debug)]
pub(crate) struct CodeLine {
    number: u32,
    text: String,
}

/// Reads a text file and converts its content into a vector of code lines.

/// The `get_text` function reads the content of a text file specified by the `input_file` path,
/// and converts each line of text into a `CodeLine` structure. The resulting `CodeLine` structures
/// are collected in a vector, making it easy to work with text content as a collection of code lines.
pub(crate) fn get_text(input_file: &str) -> Result<Vec<CodeLine>, crate::DocError> {
    let file = File::open(input_file).map_err(|_| crate::DocError::FileEditError)?;
    let reader = BufReader::new(file);
    let mut code = Vec::new();

    for (line, number) in reader.lines().zip(0u32..) {
        code.push(CodeLine { number, text: line.map_err(|_| crate::DocError::FileEditError)? });
    }

    Ok(code)
}