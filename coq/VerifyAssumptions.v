(** Verification file: prints Print Assumptions for all theorems.
    The output of coqc on this file is the assumptions check. *)
From Top Require Import BudgetTypedCap.
From Top Require Import BudgetIrisTypedCap.
From Top Require Import BudgetRustBeltTypedCap.

(* Tier A — abstract state machine *)
Print Assumptions typed_cap_soundness.
Print Assumptions typed_cap_conservation.
Print Assumptions step_invariant.
Print Assumptions reachable_invariant.

(* Tier B — Iris Hoare triples on heap_lang *)
Print Assumptions wp_spend_cap.
Print Assumptions wp_split_cap.
Print Assumptions wp_merge_cap.
Print Assumptions wp_consume_cap.
Print Assumptions budget_inv_cap_excl.
Print Assumptions budget_iris_typed_cap_closed.

(* Tier C — lambdaRust structural well-typing *)
Print Assumptions spend_well_typed_max.
Print Assumptions split_well_typed_max.
Print Assumptions merge_well_typed_max.
Print Assumptions consume_well_typed_max.
Print Assumptions rust_to_abstract_refinement_max.
