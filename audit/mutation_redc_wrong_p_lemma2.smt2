; Lemma 2 — carry lemma: for any 64-bit X with lo32(X) = 0 and any 32-bit m,
; extract 63 32 (X + zext(m)) = extract 63 32 X (no carry crosses bit 31).
; This is why a P+1 vmlal constant (which adds exactly m < 2^32 to the
; accumulated product) cannot change the REDC output's t, ge, or out.
; Negated; unsat (pure bit-vector reasoning, no multiplication).
(set-logic QF_BV)
(declare-const X (_ BitVec 64))
(declare-const m (_ BitVec 32))
(assert (= ((_ extract 31 0) X) #x00000000))
(assert (not (= ((_ extract 63 32) (bvadd X ((_ zero_extend 32) m)))
                ((_ extract 63 32) X))))
(check-sat)
(exit)
