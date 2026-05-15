(** * BudgetIrisTypedCap.v — Tier B extended to const-generic Budget<MAX>

    This file extends Tier B (BudgetIris.v) of the main mechanization
    to the const-generic [Budget<const MAX: u64>] type from the
    [budget-typed-cap] Rust crate.

    The runtime Tier B (BudgetIris.v) proves per-method Hoare triples
    over a value [budget_inv l v := l ↦ #v] with no type-level cap.
    This file lifts those triples to a [budget_inv_cap MAX l v]
    predicate that bundles the type-level invariant

        0 ≤ v ≤ MAX  ∧  MAX < 2^63

    matching the Rust const-assertion in [src/lib.rs]:

        const _A2_HOLDS: () = assert!(MAX < (1u64 << 63), ...);

    Every concrete Rust instantiation [Budget::<K>] in the source pairs
    with an Iris-level instantiation of these Hoare triples at the same
    K, with the [MAX < 2^63] hypothesis discharged by rustc's const-eval
    check at monomorphization.

    What this file proves (assuming BudgetIris.v compiles):
    - [budget_inv_cap_excl]: capped budgets are still affine.
    - [wp_spend_cap], [wp_split_cap], [wp_merge_cap], [wp_consume_cap]:
      each per-method Hoare triple preserves the type-level cap bound.

    Compile with:  coqc -Q . Top BudgetIrisTypedCap.v
    Requires: BudgetAbstract.v and BudgetIris.v (the runtime Tier B)
              to be compiled and findable under [Top].
*)

From iris.proofmode Require Import proofmode.
From iris.base_logic Require Import invariants.
From iris.heap_lang Require Import lang notation proofmode.

From Top Require Import BudgetAbstract BudgetIris.

Set Default Proof Using "Type".

Section budget_iris_typed_cap.
Context `{!heapGS Σ}.

(** ** The A2 bound. *)

(** The Rust const-assertion enforces [MAX < (1u64 << 63)]. We mirror
    this in Coq as [MAX < 2^63] with the literal value spelled out. *)
Definition A2_bound : Z := 9223372036854775808%Z.   (* = 2^63 *)

Lemma A2_bound_eq : A2_bound = (2 ^ 63)%Z.
Proof. unfold A2_bound. lia. Qed.

(** ** Capped invariant *)

(** [budget_inv_cap MAX l v] is the type-level-capped variant of
    [budget_inv l v]. It bundles three facts:
      1. The heap cell at [l] holds [v] (ownership).
      2. [0 ≤ v ≤ MAX] (the value-level invariant).
      3. [MAX < 2^63] (A2 — matches the Rust const-assertion).
*)
Definition budget_inv_cap (MAX : Z) (l : loc) (v : Z) : iProp Σ :=
  (budget_inv l v ∗ ⌜(0 ≤ v ≤ MAX)%Z⌝ ∗ ⌜(MAX < A2_bound)%Z⌝)%I.

(** ** Affineness inherited from the runtime invariant *)

Lemma budget_inv_cap_excl MAX l v v' :
  budget_inv_cap MAX l v -∗ budget_inv_cap MAX l v' -∗ False.
Proof.
  iIntros "Hc1 Hc2".
  iDestruct "Hc1" as "(Hl1 & _ & _)".
  iDestruct "Hc2" as "(Hl2 & _ & _)".
  iApply (budget_inv_excl with "Hl1 Hl2").
Qed.

(** ** Hoare triple for spend (capped) *)

(** Capped spend: the additional precondition [0 ≤ r] is required so
    that the post-condition value [v - r] cannot exceed [v] (and hence
    cannot exceed [MAX]). The success case yields a new capped invariant
    at the same location with value [v - r]; the failure case returns
    the original capped invariant unchanged. *)
Lemma wp_spend_cap (MAX : Z) (l : loc) (v r : Z) :
  (0 ≤ r)%Z →
  {{{ budget_inv_cap MAX l v }}}
    spend_fn #l #r
  {{{ (success : bool) (l' : loc), RET (#success, #l');
      (⌜success = true⌝ ∗ ⌜(r ≤ v)%Z⌝ ∗ ⌜l' = l⌝ ∗
       budget_inv_cap MAX l' (v - r)%Z)
      ∨
      (⌜success = false⌝ ∗ ⌜(v < r)%Z⌝ ∗ budget_inv_cap MAX l v) }}}.
Proof.
  iIntros (Hr Φ) "Hc HΦ".
  iDestruct "Hc" as "(Hl & %Hv & %HA2)".
  iApply (wp_spend with "Hl").
  iIntros "!>" (success l') "[Hsucc | Hfail]".
  - (* success branch *)
    iDestruct "Hsucc" as "(%Hs & %Hrv & Hl' & %Hl'eq)".
    iApply ("HΦ" $! success l').
    iLeft. iFrame "Hl'". iPureIntro.
    split_and!; (assumption || lia).
  - (* failure branch *)
    iDestruct "Hfail" as "(%Hs & %Hvr & Hl)".
    iApply ("HΦ" $! success l').
    iRight. iFrame "Hl". iPureIntro.
    split_and!; (assumption || lia).
Qed.

(** ** Hoare triple for split (capped) *)

(** Capped split: requires [0 ≤ a]. The success case yields two capped
    budgets: the original at value [v - a] and a fresh one at value
    [a]. Both satisfy the cap bound: [v - a ≤ v ≤ MAX] and [a ≤ v ≤
    MAX]. *)
Lemma wp_split_cap (MAX : Z) (l : loc) (v a : Z) :
  (0 ≤ a)%Z →
  {{{ budget_inv_cap MAX l v }}}
    split_fn #l #a
  {{{ (success : bool) (l' l'' : loc), RET (#success, #l', #l'');
      (⌜success = true⌝ ∗ ⌜(a ≤ v)%Z⌝ ∗ ⌜l' = l⌝ ∗
       budget_inv_cap MAX l' (v - a)%Z ∗ budget_inv_cap MAX l'' a)
      ∨
      (⌜success = false⌝ ∗ ⌜(v < a)%Z⌝ ∗ budget_inv_cap MAX l v) }}}.
Proof.
  iIntros (Ha Φ) "Hc HΦ".
  iDestruct "Hc" as "(Hl & %Hv & %HA2)".
  iApply (wp_split with "Hl").
  iIntros "!>" (success l' l'') "[Hsucc | Hfail]".
  - iDestruct "Hsucc" as "(%Hs & %Hav & %Hl'eq & Hl' & Hl'')".
    iApply ("HΦ" $! success l' l'').
    iLeft. iFrame "Hl' Hl''". iPureIntro.
    split_and!; (assumption || lia).
  - iDestruct "Hfail" as "(%Hs & %Hva & Hl)".
    iApply ("HΦ" $! success l' l'').
    iRight. iFrame "Hl". iPureIntro.
    split_and!; (assumption || lia).
Qed.

(** ** Hoare triple for merge (capped) *)

(** Capped merge: requires the sum [v1 + v2] to fit within MAX (and
    hence within 2^63 by A2). The post-condition yields a capped
    budget at value [v1 + v2]. *)
Lemma wp_merge_cap (MAX : Z) (l1 l2 : loc) (v1 v2 : Z) :
  (v1 + v2 ≤ MAX)%Z →
  {{{ budget_inv_cap MAX l1 v1 ∗ budget_inv_cap MAX l2 v2 }}}
    merge_fn #l1 #l2
  {{{ RET #l1; budget_inv_cap MAX l1 (v1 + v2)%Z }}}.
Proof.
  iIntros (Hsum Φ) "[Hc1 Hc2] HΦ".
  iDestruct "Hc1" as "(Hl1 & %Hv1 & %HA2)".
  iDestruct "Hc2" as "(Hl2 & %Hv2 & _)".
  iApply (wp_merge with "[$Hl1 $Hl2]").
  iIntros "!> Hl".
  iApply "HΦ".
  iFrame "Hl". iPureIntro.
  split_and!; (assumption || lia).
Qed.

(** ** Hoare triple for consume (capped) *)

(** Consume yields the value as a Z and leaves the budget at 0. The
    new value 0 trivially satisfies [0 ≤ 0 ≤ MAX]. *)
Lemma wp_consume_cap (MAX : Z) (l : loc) (v : Z) :
  {{{ budget_inv_cap MAX l v }}}
    consume_fn #l
  {{{ RET #v; budget_inv_cap MAX l 0%Z }}}.
Proof.
  iIntros (Φ) "Hc HΦ".
  iDestruct "Hc" as "(Hl & %Hv & %HA2)".
  iApply (wp_consume with "Hl").
  iIntros "!> Hl".
  iApply "HΦ".
  iFrame "Hl". iPureIntro.
  split_and!; (assumption || lia).
Qed.

(** ** Aggregate theorem: all four capped Hoare triples are closed *)

(** This is a Coq-level aggregator that simply names the four triples
    together. The honest content is in the four individual triples
    above; this theorem makes the closure-set visible from a single
    name for the paper's Data Availability listing. *)
Theorem budget_iris_typed_cap_closed (MAX : Z) :
  (forall l v r, (0 ≤ r)%Z →
     {{{ budget_inv_cap MAX l v }}} spend_fn #l #r
     {{{ (success : bool) (l' : loc), RET (#success, #l');
         (⌜success = true⌝ ∗ ⌜(r ≤ v)%Z⌝ ∗ ⌜l' = l⌝ ∗
          budget_inv_cap MAX l' (v - r)%Z)
         ∨ (⌜success = false⌝ ∗ ⌜(v < r)%Z⌝ ∗ budget_inv_cap MAX l v) }}}) /\
  (forall l v a, (0 ≤ a)%Z →
     {{{ budget_inv_cap MAX l v }}} split_fn #l #a
     {{{ (success : bool) (l' l'' : loc), RET (#success, #l', #l'');
         (⌜success = true⌝ ∗ ⌜(a ≤ v)%Z⌝ ∗ ⌜l' = l⌝ ∗
          budget_inv_cap MAX l' (v - a)%Z ∗ budget_inv_cap MAX l'' a)
         ∨ (⌜success = false⌝ ∗ ⌜(v < a)%Z⌝ ∗ budget_inv_cap MAX l v) }}}) /\
  (forall l1 l2 v1 v2, (v1 + v2 ≤ MAX)%Z →
     {{{ budget_inv_cap MAX l1 v1 ∗ budget_inv_cap MAX l2 v2 }}}
       merge_fn #l1 #l2
     {{{ RET #l1; budget_inv_cap MAX l1 (v1 + v2)%Z }}}) /\
  (forall l v,
     {{{ budget_inv_cap MAX l v }}} consume_fn #l
     {{{ RET #v; budget_inv_cap MAX l 0%Z }}}).
Proof.
  split_and!.
  - intros. apply wp_spend_cap; assumption.
  - intros. apply wp_split_cap; assumption.
  - intros. apply wp_merge_cap; assumption.
  - intros. apply wp_consume_cap.
Qed.

End budget_iris_typed_cap.

(** ** Status of this file

    What this file CLOSES of the const-generic Tier B extension:
    - The capped invariant [budget_inv_cap] bundles the value bound
      [0 ≤ v ≤ MAX] and the A2 bound [MAX < 2^63].
    - All four per-method Hoare triples lift to the capped invariant:
      they require the cap bound on entry and preserve it on exit
      (or restore the original on failure).
    - Affineness ([budget_inv_cap_excl]) is preserved.

    What this file does NOT close:
    - The RustBelt-type embedding: see BudgetRustBeltTypedCap.v.
    - The trace-level semantic refinement of the running program
      (still the principal remaining open obligation, identical to
      the runtime version).

    Pairing with the Rust const-assertion:
    Every concrete Rust instantiation [Budget::<K>::new(...)] in the
    source pairs with an Iris instantiation of these triples at MAX = K.
    The Rust const-eval check on [_A2_HOLDS] discharges the Coq
    hypothesis [MAX < A2_bound] mechanically at monomorphization;
    Coq has no way of knowing whether the K passed in was checked, but
    the rustc-side check is sound and the typing is fully recovered
    at every concrete instantiation point.
*)
