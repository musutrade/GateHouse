# Core result-boundary compatibility

These tests retain Core full-quality policy/runtime identity and saved-result/network-boundary guarantees. Set GATEHOUSE_CORE to the built Core and GATEPROBES_CONSUMER to the verified extracted github-check artifact. No GateProbes source checkout is required. The 22 platform consumer tests now live and run in GateProbes; these four paired identity tests and two network tests remain Core contract coverage. Network enforcement may explicitly skip on hosts without an unprivileged mechanism; never interpret that skip as enforcement evidence.
