(** * BudgetLinearTrace.v

    A pure-Coq formalization of "linear traces" of Budget operations:
    sequences of spend/split/merge/consume operations that respect the
    affine ownership discipline.

    This file factors the previously-Admitted "session" obligations of
    BudgetIris.v into pure Coq, so they can be discharged without
    Iris. The Iris layer (BudgetIris.v) then merely connects each
    operation to its heap-state effect via a Hoare triple.

    Status: complete, no Admitted, no axioms beyond Coq stdlib.
    Compile with: coqc -Q . Top BudgetLinearTrace.v
*)

From Coq Require Import Arith Lia List Bool.
Import ListNotations.

From Top Require Import BudgetAbstract.

(** ** Linear (affine) state and operations *)

(** A [linear_state] is a list of (budget_id, value) pairs. Each
    budget_id is a [nat] uniquely identifying a Budget instance.
    The state is finite and ordered; affineness is enforced by the
    [well_formed] predicate below. *)

Definition budget_id := nat.
Definition linear_state := list (budget_id * nat).

(** The four spend-path operations. Receipt-path operations are out
    of scope here; they are handled separately in BudgetIris.v. *)
Inductive op : Type :=
| OpSpend   : budget_id -> nat -> op             (** spend r from b *)
| OpSplit   : budget_id -> budget_id -> nat -> op (** split b into b (remainder) and fresh b' with value a *)
| OpMerge   : budget_id -> budget_id -> op       (** merge b1 and b2 into b1 *)
| OpConsume : budget_id -> op.                    (** consume b *)

(** [sum_of] : the live sum (matches the abstract liveSum). *)
Definition sum_of (s : linear_state) : nat :=
  fold_right (fun bv acc => snd bv + acc) 0 s.

(** [lookup b s] : the value of budget [b] in state [s], or [None]
    if [b] is not live. *)
Fixpoint lookup (b : budget_id) (s : linear_state) : option nat :=
  match s with
  | [] => None
  | (b', v) :: rest =>
      if Nat.eqb b b' then Some v else lookup b rest
  end.

(** [update b v s] : replace the entry for [b] in [s] with value [v]. *)
Fixpoint update (b : budget_id) (v : nat) (s : linear_state) : linear_state :=
  match s with
  | [] => []
  | (b', v') :: rest =>
      if Nat.eqb b b' then (b', v) :: rest else (b', v') :: update b v rest
  end.

(** [remove b s] : drop the entry for [b] from [s]. *)
Fixpoint remove_id (b : budget_id) (s : linear_state) : linear_state :=
  match s with
  | [] => []
  | (b', v') :: rest =>
      if Nat.eqb b b' then rest else (b', v') :: remove_id b rest
  end.

(** [fresh b s] : [b] is not currently live in [s]. *)
Definition fresh (b : budget_id) (s : linear_state) : Prop :=
  lookup b s = None.

(** Step function for one operation. Returns [None] if the operation
    is not enabled (e.g., spending more than available, splitting more
    than available, merging non-existent budgets, splitting onto a
    non-fresh location). *)

Definition exec_op (s : linear_state) (o : op) : option linear_state :=
  match o with
  | OpSpend b r =>
      match lookup b s with
      | Some v => if Nat.leb r v then Some (update b (v - r) s) else None
      | None => None
      end
  | OpSplit b b' a =>
      if Nat.eqb b b' then None
      else
        match lookup b s with
        | Some v =>
            if Nat.leb a v then
              match lookup b' s with
              | Some _ => None (** b' must be fresh *)
              | None => Some ((b', a) :: update b (v - a) s)
              end
            else None
        | None => None
        end
  | OpMerge b1 b2 =>
      if Nat.eqb b1 b2 then None
      else
        match lookup b1 s, lookup b2 s with
        | Some v1, Some v2 => Some (update b1 (v1 + v2) (remove_id b2 s))
        | _, _ => None
        end
  | OpConsume b =>
      match lookup b s with
      | Some _ => Some (remove_id b s)
      | None => None
      end
  end.

(** [exec_trace s ops] : execute a sequence of operations, returning
    [Some s_final] if all succeed, else [None]. *)
Fixpoint exec_trace (s : linear_state) (ops : list op) : option linear_state :=
  match ops with
  | [] => Some s
  | o :: rest =>
      match exec_op s o with
      | Some s' => exec_trace s' rest
      | None => None
      end
  end.

(** ** The total_charged ghost variable *)

(** [charged_of] : the total cost charged across a trace. We compute
    it from the trace + initial state rather than threading it through
    the state, to keep [linear_state] pure. *)
Fixpoint charged_of (s : linear_state) (ops : list op) : nat :=
  match ops with
  | [] => 0
  | OpSpend b r :: rest =>
      match lookup b s with
      | Some v => if Nat.leb r v then
                    r + charged_of (update b (v - r) s) rest
                  else 0
      | None => 0
      end
  | OpSplit b b' a :: rest =>
      if Nat.eqb b b' then 0
      else
        match lookup b s with
        | Some v =>
            if Nat.leb a v then
              match lookup b' s with
              | Some _ => 0
              | None => charged_of ((b', a) :: update b (v - a) s) rest
              end
            else 0
        | None => 0
        end
  | OpMerge b1 b2 :: rest =>
      if Nat.eqb b1 b2 then 0
      else
        match lookup b1 s, lookup b2 s with
        | Some v1, Some v2 => charged_of (update b1 (v1 + v2) (remove_id b2 s)) rest
        | _, _ => 0
        end
  | OpConsume b :: rest =>
      match lookup b s with
      | Some v => v + charged_of (remove_id b s) rest
      | None => 0
      end
  end.

(** ** Helper lemmas about sum_of, update, and remove_id *)

Lemma lookup_in : forall b v s,
    lookup b s = Some v -> In (b, v) s.
Proof.
  intros b v s. induction s as [|[b' v'] rest IH]; simpl; intros H.
  - discriminate.
  - destruct (Nat.eqb b b') eqn:E.
    + apply Nat.eqb_eq in E. subst. inversion H; subst. left; reflexivity.
    + right. apply IH; assumption.
Qed.

Lemma lookup_le_sum : forall b v s,
    lookup b s = Some v -> v <= sum_of s.
Proof.
  intros b v s. induction s as [|[b' v'] rest IH]; simpl; intros H.
  - discriminate.
  - destruct (Nat.eqb b b') eqn:E.
    + apply Nat.eqb_eq in E; subst. inversion H; subst. lia.
    + apply IH in H. lia.
Qed.

(** If two distinct budget_ids both look up to values, their sum is
    bounded by the total sum. This is needed for OpMerge soundness. *)
Lemma two_lookup_sum_le :
  forall b1 v1 b2 v2 s,
    b1 <> b2 ->
    lookup b1 s = Some v1 ->
    lookup b2 s = Some v2 ->
    v1 + v2 <= sum_of s.
Proof.
  intros b1 v1 b2 v2 s. revert v1 v2.
  induction s as [|[b' v'] rest IH]; intros v1 v2 Hneq H1 H2; simpl in *.
  - discriminate.
  - destruct (Nat.eqb b1 b') eqn:E1.
    + apply Nat.eqb_eq in E1. subst.
      inversion H1; subst.
      destruct (Nat.eqb b2 b') eqn:E2.
      * apply Nat.eqb_eq in E2. subst. contradiction.
      * pose proof (lookup_le_sum b2 v2 rest H2). lia.
    + destruct (Nat.eqb b2 b') eqn:E2.
      * apply Nat.eqb_eq in E2. subst.
        inversion H2; subst.
        pose proof (lookup_le_sum b1 v1 rest H1). lia.
      * pose proof (IH v1 v2 Hneq H1 H2). lia.
Qed.

Lemma update_sum : forall b v_old v_new s,
    lookup b s = Some v_old ->
    sum_of (update b v_new s) = sum_of s - v_old + v_new.
Proof.
  intros b v_old v_new s. induction s as [|[b' v'] rest IH]; simpl; intros H.
  - discriminate.
  - destruct (Nat.eqb b b') eqn:E.
    + apply Nat.eqb_eq in E. subst. inversion H; subst.
      simpl. lia.
    + simpl.
      assert (sum_of (update b v_new rest) = sum_of rest - v_old + v_new) by (apply IH; auto).
      pose proof (lookup_le_sum b v_old rest H) as Hle.
      lia.
Qed.

Lemma remove_id_sum : forall b v s,
    lookup b s = Some v ->
    sum_of (remove_id b s) = sum_of s - v.
Proof.
  intros b v s. induction s as [|[b' v'] rest IH]; simpl; intros H.
  - discriminate.
  - destruct (Nat.eqb b b') eqn:E.
    + apply Nat.eqb_eq in E. subst. inversion H; subst. lia.
    + simpl.
      assert (sum_of (remove_id b rest) = sum_of rest - v) by (apply IH; auto).
      pose proof (lookup_le_sum b v rest H) as Hle.
      lia.
Qed.

(** ** Sum + charged conservation lemma *)

(** The correct invariant: for any state [s] and remaining ops [ops],
    if execution succeeds with final state [s_final], then
    [sum_of s = sum_of s_final + charged_of s ops]. That is, the
    initial sum equals the final sum plus all charges incurred. *)

Lemma exec_op_sum_charged : forall s o s' ops,
    exec_op s o = Some s' ->
    sum_of s + charged_of s' ops = sum_of s' + charged_of s (o :: ops).
Proof.
  intros s [b r | b b' a | b1 b2 | b] s' ops H.
  - (* OpSpend *)
    cbn in H.
    destruct (lookup b s) as [v|] eqn:Hb; [|discriminate].
    destruct (Nat.leb r v) eqn:Hle_b; [|discriminate].
    inversion H; subst.
    cbn. rewrite Hb, Hle_b.
    apply Nat.leb_le in Hle_b.
    pose proof (lookup_le_sum b v s Hb) as Hsub.
    rewrite (update_sum b v (v - r) s Hb).
    lia.
  - (* OpSplit *)
    cbn in H.
    destruct (Nat.eqb b b') eqn:Heq; [discriminate|].
    destruct (lookup b s) as [v|] eqn:Hb; [|discriminate].
    destruct (Nat.leb a v) eqn:Hle_b; [|discriminate].
    destruct (lookup b' s) eqn:Hb'; [discriminate|].
    inversion H; subst.
    cbn. rewrite Heq, Hb, Hle_b, Hb'.
    apply Nat.leb_le in Hle_b.
    pose proof (lookup_le_sum b v s Hb) as Hsub.
    fold (sum_of (update b (v - a) s)).
    rewrite (update_sum b v (v - a) s Hb).
    cbn. lia.
  - (* OpMerge *)
    cbn in H.
    destruct (Nat.eqb b1 b2) eqn:Heq; [discriminate|].
    destruct (lookup b1 s) as [v1|] eqn:Hb1; [|discriminate].
    destruct (lookup b2 s) as [v2|] eqn:Hb2; [|discriminate].
    inversion H; subst.
    cbn. rewrite Heq, Hb1, Hb2.
    assert (Hlook: lookup b1 (remove_id b2 s) = Some v1).
    { clear -Hb1 Heq.
      induction s as [|[b' v'] rest IH]; cbn in *; [discriminate|].
      destruct (Nat.eqb b2 b') eqn:Eb2.
      - apply Nat.eqb_eq in Eb2. subst.
        destruct (Nat.eqb b1 b') eqn:Eb1.
        + apply Nat.eqb_eq in Eb1. subst.
          pose proof (Nat.eqb_refl b') as Hrefl.
          congruence.
        + exact Hb1.
      - cbn. destruct (Nat.eqb b1 b') eqn:Eb1.
        + exact Hb1.
        + apply IH; assumption.  }
    rewrite (update_sum b1 v1 (v1 + v2) (remove_id b2 s) Hlook).
    pose proof (lookup_le_sum b1 v1 s Hb1) as Hsub1.
    pose proof (lookup_le_sum b2 v2 s Hb2) as Hsub2.
    assert (Hneq : b1 <> b2) by (intro Habs; subst; rewrite Nat.eqb_refl in Heq; discriminate).
    pose proof (two_lookup_sum_le b1 v1 b2 v2 s Hneq Hb1 Hb2) as Hsum_both.
    rewrite (remove_id_sum b2 v2 s Hb2).
    lia.
  - (* OpConsume *)
    cbn in H.
    destruct (lookup b s) as [v|] eqn:Hb; [|discriminate].
    inversion H; subst.
    cbn. rewrite Hb.
    rewrite (remove_id_sum b v s Hb).
    pose proof (lookup_le_sum b v s Hb) as Hsub.
    lia.
Qed.

(** Trace-level: sum + total_charged is invariant. *)
Lemma exec_trace_sum_charged : forall ops s s',
    exec_trace s ops = Some s' ->
    sum_of s = sum_of s' + charged_of s ops.
Proof.
  induction ops as [|o rest IH]; intros s s' H.
  - cbn in *. inversion H; subst. lia.
  - cbn in H.
    destruct (exec_op s o) as [s_mid|] eqn:Hop; [|discriminate].
    pose proof (exec_op_sum_charged _ _ _ rest Hop) as Hstep.
    apply IH in H.
    (* H : sum_of s_mid = sum_of s' + charged_of s_mid rest *)
    (* Hstep : sum_of s + charged_of s_mid rest = sum_of s_mid + charged_of s (o :: rest) *)
    (* Goal: sum_of s = sum_of s' + charged_of s (o :: rest) *)
    lia.
Qed.

(** ** The headline theorem: linear-trace cap soundness *)

Definition initial_linear_state (B0 : nat) : linear_state := [(0, B0)].

Theorem linear_trace_cap_soundness :
  forall B0 ops s_final,
    exec_trace (initial_linear_state B0) ops = Some s_final ->
    charged_of (initial_linear_state B0) ops <= B0.
Proof.
  intros B0 ops s_final H.
  pose proof (exec_trace_sum_charged _ _ _ H) as Hsum.
  unfold initial_linear_state in *. cbn in Hsum.
  (* Hsum : B0 + 0 = sum_of s_final + charged_of [(0, B0)] ops *)
  (* So B0 = sum_of s_final + charged_of [(0, B0)] ops. *)
  (* Since sum_of s_final >= 0, we get charged_of <= B0. *)
  lia.
Qed.

(** ** Bridge to BudgetAbstract.reachable *)

(** Each linear-trace operation corresponds to a sequence of abstract
    state machine transitions. Specifically:
    - OpSpend r : ASpendSuccess r
    - OpSplit b b' a : no abstract change (split preserves liveSum)
    - OpMerge b1 b2 : no abstract change (merge preserves liveSum)
    - OpConsume b : AConsume (with the consumed liveSum going to totalReleased) *)

(** [abstract_state_of B0 s ops] : the abstract state after executing
    [ops] from initial state derived from [s]. *)
Definition abstract_state_of (B0 : nat) (s : linear_state) (charged : nat) : state :=
  {| liveSum := sum_of s;
     outstandingReceipts := 0;
     outstandingRefunds := 0;
     totalCharged := charged;
     totalUnrecoverable := 0;
     totalReleased := B0 - sum_of s - charged |}.

Lemma abstract_state_initial : forall B0,
  abstract_state_of B0 (initial_linear_state B0) 0 = initial B0.
Proof.
  intros B0. unfold abstract_state_of, initial_linear_state, initial.
  cbn. f_equal; lia.
Qed.

(** ** Status

    All Admitted obligations from the original BudgetIris.v
    "session" machinery are now discharged in pure Coq:
    - exec_op_sum_charged    : ✓ proved
    - exec_trace_sum_charged : ✓ proved
    - linear_trace_cap_soundness : ✓ proved (headline)
    - abstract_state_initial : ✓ proved

    [Print Assumptions linear_trace_cap_soundness] should report
    "Closed under the global context" — no axioms used.
*)

Print Assumptions linear_trace_cap_soundness.
