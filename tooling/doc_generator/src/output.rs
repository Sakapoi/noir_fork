use std::fmt;

use noirc_frontend::token::{DocStyle, Keyword, SpannedToken, Token};

use crate::{
    additional_doc, doc, fn_signature, get_module_content, outer_doc, skip_impl_block,
    struct_signature, trait_info, Function, Implementation,
};

/// Represents the type or category of code element or information.

/// The `Type` enum is used to categorize code elements or information based on their type or purpose.
/// This classification can help in organizing and processing code elements and their associated documentation.
#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub(crate) enum Type {
    Function,
    Module,
    Struct,
    Trait,
    OuterComment,
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Function => writeln!(f, "Function"),
            Type::Module => writeln!(f, "Module"),
            Type::Struct => writeln!(f, "Struct"),
            Type::Trait => writeln!(f, "Trait"),
            Type::OuterComment => writeln!(f, "OuterComment"),
        }
    }
}

/// Represents detailed information about a code element.

/// The `Info` enum provides detailed information about code elements, including their type, signature,
/// documentation, and any additional details. It is used to capture and organize information related to code elements
/// for documentation and processing purposes.
#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub(crate) enum Info {
    Function {
        signature: String,
    },
    Module {
        content: Vec<Output>,
    },
    Struct {
        signature: String,
        additional_doc: String,
        implementations: Vec<Implementation>,
    },
    Trait {
        signature: String,
        additional_doc: String,
        required_methods: Vec<Function>,
        provided_methods: Vec<Function>,
        implementations: Vec<Implementation>,
    },
    Blanc,
}

impl Info {
    pub(crate) fn get_signature(&self) -> Option<String> {
        match self {
            Info::Function { signature } => Some(signature.to_string()),
            Info::Struct { signature, .. } => Some(signature.to_string()),
            Info::Trait { signature, .. } => Some(signature.to_string()),
            _ => None,
        }
    }

    pub(crate) fn get_implementations(&self) -> Option<Vec<Implementation>> {
        match self {
            Info::Struct { implementations, .. } => Some(implementations.clone()),
            Info::Trait { implementations, .. } => Some(implementations.clone()),
            _ => None,
        }
    }

    pub(crate) fn get_additional_doc(&self) -> Option<String> {
        match self {
            Info::Struct { additional_doc, .. } => Some(additional_doc.to_string()),
            Info::Trait { additional_doc, .. } => Some(additional_doc.to_string()),
            _ => None,
        }
    }

    pub(crate) fn get_required_methods(&self) -> Option<Vec<Function>> {
        match self {
            Info::Trait { required_methods, .. } => Some(required_methods.clone()),
            _ => None,
        }
    }

    pub(crate) fn get_provided_methods(&self) -> Option<Vec<Function>> {
        match self {
            Info::Trait { provided_methods, .. } => Some(provided_methods.clone()),
            _ => None,
        }
    }

    pub(crate) fn get_content(&self) -> Option<Vec<Output>> {
        match self {
            Info::Module { content } => Some(content.clone()),
            _ => None,
        }
    }
}

/// Represents an output object that combines code information, documentation, and type details.

/// The `Output` struct serves as a container for code-related information, documentation, and type details.
/// It allows you to bundle these attributes together, making it convenient for storing and processing code
/// elements, their associated documentation, and their categorized types.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub(crate) struct Output {
    pub(crate) r#type: Type,
    pub(crate) name: String,
    pub(crate) doc: String,
    pub(crate) information: Info,
}

impl Output {
    /// Converts a vector of spanned tokens into a vector of structured output objects.

    /// The `to_output` function processes a vector of spanned tokens, typically representing source code,
    /// and converts them into a vector of structured output objects. Each output object includes information
    /// about the code element, its type, name, documentation, and additional details as applicable.
    pub(crate) fn to_output(input: Vec<SpannedToken>) -> Result<Vec<Self>, crate::DocError> {
        let mut res = Vec::new();
        let tokens = input.into_iter().map(|x| x.into_token()).collect::<Vec<_>>();
        let mut is_first = true;
        let mut skip_count = 0;

        for i in 0..tokens.len() {
            if skip_count > 0 {
                skip_count -= 1;
                continue;
            }
            let out = match &tokens[i] {
                Token::Keyword(Keyword::Fn) => {
                    let r#type = Type::Function;
                    let name = match &tokens[i + 1] {
                        Token::Ident(idn) => idn.clone(),
                        _ => {
                            continue;
                        }
                    };
                    let doc = doc(&tokens, i);
                    let sign = fn_signature(&tokens, i);

                    Output { r#type, name, doc, information: Info::Function { signature: sign } }
                }
                Token::Keyword(Keyword::Struct) => {
                    let r#type = Type::Struct;
                    let name = match &tokens[i + 1] {
                        Token::Ident(idn) => idn.clone(),
                        _ => {
                            continue;
                        }
                    };
                    let doc = doc(&tokens, i);
                    let sign = struct_signature(&tokens, i);
                    let ad_doc = additional_doc(&tokens, i);

                    Output {
                        r#type,
                        name: name.clone(),
                        doc,
                        information: Info::Struct {
                            signature: sign,
                            additional_doc: ad_doc,
                            implementations: Implementation::get_implementations(&tokens, i, name),
                        },
                    }
                }
                Token::Keyword(Keyword::Trait) => {
                    skip_count = skip_impl_block(&tokens, i);

                    let r#type = Type::Trait;
                    let name = match &tokens[i + 1] {
                        Token::Ident(idn) => idn.clone(),
                        _ => {
                            continue;
                        }
                    };
                    let doc = doc(&tokens, i);

                    let ad_doc = additional_doc(&tokens, i);
                    let impls = Implementation::get_implementations(&tokens, i, name.clone());
                    let info = trait_info(&tokens, i);

                    Output {
                        r#type,
                        name,
                        doc,
                        information: Info::Trait {
                            signature: info.0,
                            additional_doc: ad_doc,
                            required_methods: info.1,
                            provided_methods: info.2,
                            implementations: impls,
                        },
                    }
                }
                Token::Keyword(Keyword::Mod) => {
                    if tokens[i + 2] == Token::LeftBrace {
                        skip_count = skip_impl_block(&tokens, i);
                    }

                    let r#type = Type::Module;
                    let name = match &tokens[i + 1] {
                        Token::Ident(idn) => idn.clone(),
                        _ => {
                            continue;
                        }
                    };
                    let doc = doc(&tokens, i);
                    let content = get_module_content(&tokens, i);

                    Output { r#type, name, doc, information: Info::Module { content: content? } }
                }
                Token::LineComment(_, Some(DocStyle::Inner))
                | Token::BlockComment(_, Some(DocStyle::Inner)) => {
                    let r#type = Type::OuterComment;
                    let name = "".to_string();

                    let res = outer_doc(&tokens, i);

                    let doc = if is_first {
                        is_first = false;
                        res.0
                    } else {
                        if res.1 == i {
                            is_first = true;
                        }
                        "".to_string()
                    };

                    Output { r#type, name, doc, information: Info::Blanc }
                }
                Token::Keyword(Keyword::Impl) => {
                    skip_count = skip_impl_block(&tokens, i);
                    continue;
                }
                _ => {
                    continue;
                }
            };

            res.push(out);
        }

        Ok(res)
    }
}

impl fmt::Display for Output {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Type: {:?}\n", self.r#type)?;
        writeln!(f, "Name: {}\n", self.name)?;
        writeln!(f, "Doc: {}\n", self.doc)?;
        Ok(())
    }
}