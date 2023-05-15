use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens, TokenStreamExt};
use syn::Ident;

use regexlib::automata::{Automaton, State, TransitionCondition};
use regexlib::parser::character_class::CharacterClass;

pub struct AdvanceFunction {
    advance_expressions: Vec<AdvanceExpression>,
}

impl AdvanceFunction {
    pub fn new(automaton: &Automaton) -> Self {
        let advance_expressions = automaton
            .states()
            .iter()
            .map(|state| AdvanceExpression::new(automaton, state))
            .collect();

        Self {
            advance_expressions,
        }
    }
}

impl ToTokens for AdvanceFunction {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let advance_expressions = &self.advance_expressions;
        tokens.append_all(quote! {
            fn advance(&self, next: char) -> Self{
                Self{
                    #(#advance_expressions)*
                }
            }
        });
    }
}

#[derive(Debug)]
struct AdvanceExpression {
    state_id: usize,
    transition_tos: Vec<TransitionTo>,
}

impl AdvanceExpression {
    fn new(automaton: &Automaton, target_state: &State) -> Self {
        let transition_tos = automaton
            .states()
            .iter()
            .flat_map(|previous_state| {
                previous_state.transitions.iter().filter_map(|transition| {
                    if transition.next_state_id == target_state.id {
                        Some(TransitionTo {
                            previous_state_id: previous_state.id,
                            condition: transition.condition.clone(),
                        })
                    } else {
                        None
                    }
                })
            })
            .collect::<Vec<_>>();

        Self {
            state_id: target_state.id,
            transition_tos,
        }
    }
}

impl ToTokens for AdvanceExpression {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let state_variable_name = Ident::new(&format!("state{}", self.state_id), Span::call_site());
        let transition_tos = &self.transition_tos;
        if transition_tos.is_empty() {
            tokens.append_all(quote! {#state_variable_name: false,});
        } else {
            tokens.append_all(quote! {#state_variable_name: #((#transition_tos))||*,});
        }
    }
}

#[derive(Debug)]
struct TransitionTo {
    previous_state_id: usize,
    condition: TransitionCondition,
}

impl ToTokens for TransitionTo {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let inner_condition = match &self.condition {
            TransitionCondition::AnyCharacter => quote!(true),
            TransitionCondition::Literal(must_match) => quote!(next == #must_match),
            TransitionCondition::CharacterClass(character_class) => {
                character_class_to_token_stream(character_class)
            }
            _ => unimplemented!(),
        };

        let previous_state_name = Ident::new(
            &format!("state{}", self.previous_state_id),
            Span::call_site(),
        );

        tokens.append_all(quote!(self.#previous_state_name && (#inner_condition)));
    }
}

fn character_class_to_token_stream(character_class: &CharacterClass) -> TokenStream {
    match character_class {
        CharacterClass::Char(ch) => quote!(next == #ch),
        CharacterClass::Negated(class) => {
            let base = character_class_to_token_stream(class);
            quote!(!(#base))
        }
        CharacterClass::Range { start, end } => {
            quote!(((next as u32) >= (#start as u32)) && ((next as u32) <= (#end as u32)))
        }
        CharacterClass::Disjunction(classes) => {
            let token_streams = classes
                .iter()
                .map(character_class_to_token_stream)
                .collect::<Vec<_>>();
            quote!(#((#token_streams))||*)
        }
    }
}
