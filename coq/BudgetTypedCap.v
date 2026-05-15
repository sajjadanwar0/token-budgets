(* =============================================================== *)
(*  BudgetTypedCap.v                                                 *)
(*                                                                   *)
(*  Cap-soundness mechanization for the const-generic Budget<MAX>    *)
(*  type from the `budget-typed-cap` Rust crate.                     *)
(*                                                                   *)
(*  Theorem typed_cap_soundness:                                     *)
(*    forall (MAX : Z) (s : state),                                  *)
(*      0 <= MAX < 2^63 ->                                           *)
(*      reachable MAX s ->                                           *)
(*      committed s <= MAX.                                          *)
(*                                                                   *)
(*  The hypothesis `MAX < 2^63` mirrors the Rust const-assertion     *)
(*                                                                   *)
(*    const _A2_HOLDS: () = assert!(MAX < (1u64 << 63), ...);        *)
(*                                                                   *)
(*  in `budget-typed-cap/src/lib.rs`. Every concrete instantiation   *)
(*  `Budget::<K>` in Rust is paired with a concrete instantiation    *)
(*  of this theorem at the same K, with the Coq hypothesis           *)
(*  discharged by the const-eval check rustc performs at             *)
(*  monomorphization.                                                *)
(*                                                                   *)
(*  Status: zero Admitted, zero Axiom. Closed under the global       *)
(*  context. Proof verified with Coq 8.18.0.                         *)
(*                                                                   *)
(*  Scope: Tier-A-level abstract state machine. Does NOT extend the  *)
(*  Tier C (lambda-rust / RustBelt) mechanization to the const-      *)
(*  generic Rust type --- that requires modeling Rust's const-       *)
(*  generic monomorphization in RustBelt, identified as open work.   *)
(* =============================================================== *)

From Coq Require Import ZArith Lia List Bool.
Import ListNotations.
Open Scope Z_scope.

Definition uc := Z.

(* =============================================================== *)
(* The A2 bound, both as a literal and as a 2^63 form.               *)
(* =============================================================== *)

Definition A2_bound : Z := 9223372036854775808.   (* = 2^63 *)

Lemma A2_bound_eq : A2_bound = 2 ^ 63.
Proof. unfold A2_bound. lia. Qed.

(* =============================================================== *)
(* Abstract budget state for cap MAX:                                *)
(*   live      : multiset of outstanding (uncommitted) budget values *)
(*   committed : cumulative spent so far, monotone                   *)
(* =============================================================== *)

Record state := mkState {
  live      : list uc;
  committed : uc;
}.

Definition sum_live (s : state) : uc :=
  fold_right Z.add 0 (live s).

Definition total (s : state) : uc :=
  sum_live s + committed s.

(* Initial state for a Budget<MAX>::new(MAX). *)
Definition init (MAX : uc) : state :=
  mkState [MAX] 0.

(* =============================================================== *)
(* Operations on Budget<MAX>.                                        *)
(* =============================================================== *)

Inductive op : Type :=
| Spend (idx : nat) (amount : uc)
| Split (idx : nat) (amount : uc)
| Merge (i j : nat).

(* List helpers. *)
Fixpoint remove_at {A} (xs : list A) (n : nat) : list A :=
  match xs, n with
  | [],      _      => []
  | _ :: xs', O     => xs'
  | x :: xs', S n'  => x :: remove_at xs' n'
  end.

Fixpoint nth_opt {A} (xs : list A) (n : nat) : option A :=
  match xs, n with
  | [],      _     => None
  | x :: _,  O     => Some x
  | _ :: xs', S n' => nth_opt xs' n'
  end.

(* Step relation. Invalid operations leave the state unchanged       *)
(* (the affine discipline in Rust prevents re-issuing the moved-     *)
(* from value, so this no-op is harmless in the model).              *)
Definition step (MAX : uc) (s : state) (o : op) : state :=
  match o with
  | Spend idx amt =>
      match nth_opt (live s) idx with
      | None       => s
      | Some v     =>
          if andb (0 <=? amt) (amt <=? v) then
            mkState
              ((v - amt) :: remove_at (live s) idx)
              (committed s + amt)
          else s
      end
  | Split idx amt =>
      match nth_opt (live s) idx with
      | None       => s
      | Some v     =>
          if andb (0 <=? amt) (amt <=? v) then
            mkState
              (amt :: (v - amt) :: remove_at (live s) idx)
              (committed s)
          else s
      end
  | Merge i j =>
      if Nat.eqb i j then s else
      match nth_opt (live s) i, nth_opt (live s) j with
      | Some vi, Some vj =>
          if (vi + vj) <=? MAX then
            let removed_one  := remove_at (live s) (max i j) in
            let removed_both := remove_at removed_one (min i j) in
            mkState (vi + vj :: removed_both) (committed s)
          else s
      | _, _ => s
      end
  end.

(* =============================================================== *)
(* The invariant we will prove.                                      *)
(* =============================================================== *)

Definition invariant (MAX : uc) (s : state) : Prop :=
  total s <= MAX /\ committed s >= 0 /\ Forall (fun v => v >= 0) (live s).

(* Reachability. *)
Inductive reachable (MAX : uc) : state -> Prop :=
| reach_init  : reachable MAX (init MAX)
| reach_step  : forall s o, reachable MAX s -> reachable MAX (step MAX s o).

(* =============================================================== *)
(* Helper lemmas about the list operations.                          *)
(* =============================================================== *)

Lemma sum_live_nonneg : forall xs,
  Forall (fun v => v >= 0) xs ->
  fold_right Z.add 0 xs >= 0.
Proof.
  induction xs as [|x xs IH]; intros H; simpl.
  - lia.
  - inversion H; subst. specialize (IH H3). lia.
Qed.

Lemma nth_opt_in : forall {A} (xs : list A) n x,
  nth_opt xs n = Some x -> In x xs.
Proof.
  intros A xs. induction xs as [|h xs IH]; intros n x H; destruct n; simpl in *.
  - discriminate.
  - discriminate.
  - inversion H; subst. left. reflexivity.
  - right. eapply IH. eassumption.
Qed.

Lemma forall_nth : forall {A} (P : A -> Prop) xs n x,
  Forall P xs -> nth_opt xs n = Some x -> P x.
Proof.
  intros A P xs n x HF Hn.
  apply nth_opt_in in Hn.
  rewrite Forall_forall in HF. apply HF. assumption.
Qed.

Lemma sum_remove_at : forall xs n v,
  nth_opt xs n = Some v ->
  fold_right Z.add 0 (remove_at xs n) = fold_right Z.add 0 xs - v.
Proof.
  induction xs as [|x xs IH]; intros n v Hn; destruct n; simpl in *.
  - discriminate.
  - discriminate.
  - inversion Hn; subst. lia.
  - rewrite (IH _ _ Hn). lia.
Qed.

Lemma forall_remove_at : forall {A} (P : A -> Prop) xs n,
  Forall P xs -> Forall P (remove_at xs n).
Proof.
  intros A P xs. induction xs as [|x xs IH]; intros n; destruct n; simpl.
  - constructor.
  - constructor.
  - inversion 1; subst. assumption.
  - inversion 1; subst. constructor. assumption. apply IH. assumption.
Qed.

(* Removing two distinct indices: the sum drops by both values. *)
Lemma sum_remove_two : forall xs i j vi vj,
  (i < j)%nat ->
  nth_opt xs i = Some vi ->
  nth_opt xs j = Some vj ->
  fold_right Z.add 0 (remove_at (remove_at xs j) i) =
  fold_right Z.add 0 xs - vi - vj.
Proof.
  intros xs i j vi vj Hlt Hi Hj.
  assert (Hi' : nth_opt (remove_at xs j) i = Some vi).
  { generalize dependent j. generalize dependent i.
    induction xs as [|h tl IH]; intros i Hi j Hj Hlt.
    - destruct i; discriminate.
    - destruct j as [|j']; [lia|].
      simpl. destruct i as [|i']; simpl in *.
      + assumption.
      + apply (IH i' Hi j'); auto. lia. }
  rewrite (sum_remove_at _ _ _ Hi').
  rewrite (sum_remove_at _ _ _ Hj). lia.
Qed.

Lemma forall_remove_at_twice : forall {A} (P : A -> Prop) xs m n,
  Forall P xs -> Forall P (remove_at (remove_at xs m) n).
Proof.
  intros. apply forall_remove_at. apply forall_remove_at. assumption.
Qed.

(* =============================================================== *)
(* Initial state satisfies the invariant.                            *)
(* =============================================================== *)

Lemma init_invariant : forall MAX, 0 <= MAX -> invariant MAX (init MAX).
Proof.
  intros MAX HM. unfold invariant, init, total, sum_live. simpl.
  repeat split.
  - lia.
  - lia.
  - constructor. lia. constructor.
Qed.

(* =============================================================== *)
(* Step preserves the invariant.                                     *)
(*                                                                   *)
(* Tactic strategy: avoid `simpl` until we are inside each subgoal.  *)
(* For each case (Spend / Split / Merge), build up the relevant      *)
(* arithmetic facts BEFORE attempting to split or unfold the goal.   *)
(* =============================================================== *)

Lemma step_invariant : forall MAX s o,
  invariant MAX s -> invariant MAX (step MAX s o).
Proof.
  intros MAX [livel commit] o HI. (* destruct s upfront *)
  destruct HI as [HT [HC HF]].
  unfold invariant, total, sum_live in HT, HC, HF.
  simpl in HT, HC, HF.
  pose proof (sum_live_nonneg _ HF) as Hslnn. simpl in Hslnn.
  destruct o as [idx amt | idx amt | i j].

  (* =================== Spend case =================== *)
  - cbn [step live committed].
    destruct (nth_opt livel idx) as [v|] eqn:Hn;
      [|repeat split; [unfold total, sum_live; simpl; lia | assumption | assumption]].
    destruct (andb (0 <=? amt) (amt <=? v)) eqn:Hb;
      [|repeat split; [unfold total, sum_live; simpl; lia | assumption | assumption]].
    apply Bool.andb_true_iff in Hb. destruct Hb as [Hb1 Hb2].
    apply Z.leb_le in Hb1, Hb2.
    pose proof (forall_nth _ _ _ _ HF Hn) as Hv.
    pose proof (sum_remove_at _ _ _ Hn) as Hrem.
    unfold invariant, total, sum_live. simpl.
    repeat split.
    + replace (fold_right Z.add 0 (remove_at livel idx))
         with (fold_right Z.add 0 livel - v) by assumption.
      remember (fold_right Z.add 0 livel) as L.
      lia.
    + lia.
    + constructor; [lia | apply forall_remove_at; assumption].

  (* =================== Split case =================== *)
  - cbn [step live committed].
    destruct (nth_opt livel idx) as [v|] eqn:Hn;
      [|repeat split; [unfold total, sum_live; simpl; lia | assumption | assumption]].
    destruct (andb (0 <=? amt) (amt <=? v)) eqn:Hb;
      [|repeat split; [unfold total, sum_live; simpl; lia | assumption | assumption]].
    apply Bool.andb_true_iff in Hb. destruct Hb as [Hb1 Hb2].
    apply Z.leb_le in Hb1, Hb2.
    pose proof (forall_nth _ _ _ _ HF Hn) as Hv.
    pose proof (sum_remove_at _ _ _ Hn) as Hrem.
    unfold invariant, total, sum_live. simpl.
    repeat split.
    + replace (fold_right Z.add 0 (remove_at livel idx))
         with (fold_right Z.add 0 livel - v) by assumption.
      remember (fold_right Z.add 0 livel) as L. lia.
    + assumption.
    + constructor; [lia | constructor; [lia | apply forall_remove_at; assumption]].

  (* =================== Merge case =================== *)
  - cbn [step live committed]. destruct (Nat.eqb i j) eqn:Hij.
    { repeat split; [unfold total, sum_live; simpl; lia | assumption | assumption]. }
    destruct (nth_opt livel i) as [vi|] eqn:Hni;
      [|repeat split; [unfold total, sum_live; simpl; lia | assumption | assumption]].
    destruct (nth_opt livel j) as [vj|] eqn:Hnj;
      [|repeat split; [unfold total, sum_live; simpl; lia | assumption | assumption]].
    destruct (vi + vj <=? MAX) eqn:Hsum;
      [|repeat split; [unfold total, sum_live; simpl; lia | assumption | assumption]].
    apply Z.leb_le in Hsum.
    apply Nat.eqb_neq in Hij.
    pose proof (forall_nth _ _ _ _ HF Hni) as Hvi.
    pose proof (forall_nth _ _ _ _ HF Hnj) as Hvj.
    assert (Hcombined :
      fold_right Z.add 0 (remove_at (remove_at livel (max i j)) (min i j))
      = fold_right Z.add 0 livel - vi - vj).
    { destruct (Nat.lt_ge_cases i j) as [Hlt | Hge].
      - rewrite (Nat.max_r _ _ (Nat.lt_le_incl _ _ Hlt)).
        rewrite (Nat.min_l _ _ (Nat.lt_le_incl _ _ Hlt)).
        apply sum_remove_two with (vi:=vi) (vj:=vj); assumption.
      - assert (Hgt : (j < i)%nat) by lia.
        rewrite (Nat.max_l _ _ Hge).
        rewrite (Nat.min_r _ _ Hge).
        pose proof (sum_remove_two _ _ _ _ _ Hgt Hnj Hni) as Hsr.
        (* Goal: fold_right ... = L - vi - vj.
           Hsr says the same fold_right = L - vj - vi.
           Chain by transitivity, then lia handles the commutativity. *)
        transitivity (fold_right Z.add 0 livel - vj - vi).
        + exact Hsr.
        + lia. }
    unfold invariant, total, sum_live. simpl.
    repeat split.
    + replace (fold_right Z.add 0 (remove_at (remove_at livel (Init.Nat.max i j)) (Init.Nat.min i j)))
         with (fold_right Z.add 0 livel - vi - vj) by assumption.
      remember (fold_right Z.add 0 livel) as L in *.
      assert (Heq : vi + vj + (L - vi - vj) + commit = L + commit) by ring.
      rewrite Heq. exact HT.
    + assumption.
    + simpl in Hvi, Hvj.
      constructor; [lia | apply forall_remove_at_twice; assumption].
Qed.

(* =============================================================== *)
(* Reachable states satisfy the invariant.                           *)
(* =============================================================== *)

Lemma reachable_invariant : forall MAX s,
  0 <= MAX -> reachable MAX s -> invariant MAX s.
Proof.
  intros MAX s HM Hr. induction Hr.
  - apply init_invariant. assumption.
  - apply step_invariant. assumption.
Qed.

(* =============================================================== *)
(* The main theorem: cap-soundness for Budget<MAX>.                  *)
(* =============================================================== *)

Theorem typed_cap_soundness :
  forall (MAX : uc) (s : state),
    0 <= MAX < 2 ^ 63 ->
    reachable MAX s ->
    committed s <= MAX.
Proof.
  intros MAX s [HM1 _HA2] Hr.
  pose proof (reachable_invariant _ _ HM1 Hr) as [HT [HC HF]].
  pose proof (sum_live_nonneg _ HF) as Hsl.
  unfold total, sum_live in HT. lia.
Qed.

(* =============================================================== *)
(* Conservation corollary: total (live + committed) <= MAX.          *)
(* =============================================================== *)

Theorem typed_cap_conservation :
  forall (MAX : uc) (s : state),
    0 <= MAX < 2 ^ 63 ->
    reachable MAX s ->
    total s <= MAX.
Proof.
  intros MAX s [HM1 _HA2] Hr.
  pose proof (reachable_invariant _ _ HM1 Hr) as [HT _]. assumption.
Qed.

(* =============================================================== *)
(* Print Assumptions: confirm zero axioms / admitted.                *)
(* =============================================================== *)

(* Uncomment to verify; coqc will print: Closed under the global    *)
(* context, indicating the proof has no axioms.                      *)
(*                                                                   *)
(*   Print Assumptions typed_cap_soundness.                          *)
(*   Print Assumptions typed_cap_conservation.                       *)
