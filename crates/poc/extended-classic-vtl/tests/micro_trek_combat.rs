use vtlxx_poc_extended_classic_vtl::Machine;

const SOURCE: &str = include_str!("../examples/micro_trek.vtl");
const PREFIX: &[i16] = &[-32768, 42, 32767];

fn machine() -> Machine {
    let mut m = Machine::new();
    m.execute_source("I=0 *=I<240 [ @=I,I+1000 I=I+1 ]")
        .unwrap();
    let (definitions, _) = SOURCE.split_once("; EC08_TEST_DEFINITIONS_END").unwrap();
    m.execute_source(definitions).unwrap();
    m.execute_source("E=105 F=106 G=107 H=108 I=109 J=110 K=111 L=112 M=113 N=114 O=115 P=116 Q=117 R=118 S=119 T=120 U=121 V=122 W=123 X=124 Y=125 Z=126").unwrap();
    for &value in PREFIX {
        m.push(value);
    }
    m
}

fn snapshot(m: &Machine) -> Vec<i16> {
    (0..240).map(|i| m.storage(i)).collect()
}

#[test]
fn hit_calculation_uses_safe_arithmetic_and_preserves_state() {
    for (sx, sy, ex, ey) in [(1, 1, 1, 1), (1, 1, 8, 8), (3, 4, 4, 4)] {
        for slot in [1, 6] {
            for power in [0, 1, 149, 1093, 4000] {
                let mut m = machine();
                m.execute_source(&format!(
                    "@=167,{sx} @=168,{sy} @={},300 @={},{ex} @={},{ey}",
                    134 + slot,
                    140 + slot,
                    146 + slot
                ))
                .unwrap();
                let before = snapshot(&m);
                m.execute_source(&format!("~={power} h={slot}")).unwrap();
                let d = (ex - sx) * (ex - sx) + (ey - sy) * (ey - sy) + 30;
                assert_eq!(
                    m.stack(),
                    &[PREFIX, &[(30 * power / d + 1) as i16]].concat()
                );
                assert_eq!(snapshot(&m), before);
                for i in 6..26 {
                    assert_eq!(m.register(i), Some(i as i16 + 101));
                }
                assert!(m.output().is_empty());
            }
        }
    }
}

#[test]
fn apply_hit_updates_only_owned_state_and_leaves_slot_holes() {
    for slot in [1, 6] {
        for hp in [1, 2, 300] {
            for damage in [0, 1, 299, 300] {
                for observed in [false, true] {
                    let mut m = machine();
                    let galaxy = if observed { 1317 } else { 317 };
                    m.execute_source(&format!(
                        "@=165,2 @=166,3 @=10,{galaxy} @=163,3 @=169,3 @=135,200 @=136,0 @=137,200 @=138,0 @=139,0 @=140,200 @={},{} @={},4 @={},5 @=99,1",
                        134 + slot, hp, 140 + slot, 146 + slot
                    ))
                    .unwrap();
                    let mut expected = snapshot(&m);
                    let remaining = hp - damage;
                    if remaining <= 1 {
                        expected[(134 + slot) as usize] = 0;
                        expected[99] = 0;
                        expected[10] -= 100;
                        expected[163] -= 1;
                        expected[169] -= 1;
                    } else {
                        expected[(134 + slot) as usize] = remaining as i16;
                    }
                    m.execute_source(&format!("~={slot} k={damage}")).unwrap();
                    assert_eq!(m.stack(), PREFIX);
                    assert_eq!(
                        snapshot(&m),
                        expected,
                        "slot={slot} hp={hp} damage={damage}"
                    );
                    for i in 4..26 {
                        assert_eq!(m.register(i), Some(i as i16 + 101));
                    }
                    assert!(m.output().is_empty());
                }
            }
        }
    }
}

#[test]
fn last_enemy_kill_reaches_zero_counts() {
    let mut m = machine();
    m.execute_source("@=165,1 @=166,1 @=0,1102 @=163,1 @=169,1 @=135,2 @=141,1 @=147,2 @=72,1")
        .unwrap();
    m.execute_source("~=1 k=1").unwrap();
    assert_eq!(m.stack(), PREFIX);
    for address in [135, 72, 163, 169] {
        assert_eq!(m.storage(address), 0);
    }
    assert_eq!(m.storage(0), 1002);
}
