use crate::automaton::state_enum::StateEnum;
use proc_macro2::TokenStream;
use quote::quote;
use regexlib::automata::{Automaton, State, Transition, TransitionCondition};
use regexlib::parser::character_class::CharacterClass;

pub fn emit_advance_function(automaton: &Automaton, state_enum: &StateEnum) -> TokenStream {
    let transition_handlers = automaton
        .states()
        .iter()
        .map(|state| emit_state_handler(state, state_enum))
        .collect::<Vec<_>>();

    quote! {
        pub fn advance_from(&mut self, from: &Automaton, next: char, scratch: &mut ScratchSpace){
            scratch.did_add_state_value += 1;

            self.valid_state_count = 0;

            for from_state in from.states.iter().take(from.valid_state_count){
                match from_state{
                    #(#transition_handlers)*
                }
            }
        }
    }
}

fn emit_state_handler(state: &State, state_enum: &StateEnum) -> TokenStream {
    let state_identifier = state_enum.reference_state(state);

    let transition_handlers = state
        .transitions
        .iter()
        .map(|transition| emit_state_transition_handler(transition, state_enum))
        .collect::<Vec<_>>();

    quote! {
        #state_identifier => {
            #(#transition_handlers)*
        },
    }
}

fn emit_state_transition_handler(transition: &Transition, state_enum: &StateEnum) -> TokenStream {
    let condition_checker = match &transition.condition {
        TransitionCondition::Literal(literal) => {
            let literal = *literal;
            quote! { next == #literal }
        }
        TransitionCondition::CharacterClass(class) => character_class_to_token_stream(class),
        TransitionCondition::AnyCharacter => quote! { true },
        _ => unimplemented!(),
    };

    let target_state_id = transition.next_state_id;
    let target_state_ident = state_enum.reference_id(transition.next_state_id);

    quote! {
        if (scratch.did_add_state[#target_state_id] != scratch.did_add_state_value) && (#condition_checker) {
            scratch.did_add_state[#target_state_id] = scratch.did_add_state_value;
            self.states[self.valid_state_count] = #target_state_ident;
            self.valid_state_count += 1;
        }
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
