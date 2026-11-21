# **FlexCore roadmap research: Organizational energy assessment vs. household solar planning**

**My conclusion after the second research pass is that we should retain the organizational extension, but we should also seriously consider a household Solar + Battery Planning application.**

However, I would change how we define both.

I would not build a generic energy-auditing platform, and I would not build another basic solar calculator. Both markets already have substantial existing solutions, including free ones.

There is a more specific opportunity in each market.

**The two opportunities**

[商业太阳能电池储能案例研究 - Intone Power](https://images.openai.com/static-rsc-4/UaS7QJkUk0C-9FKaAiIsPMqA6YN4bfM7RoXcLHlPkhVdCE_hzlBD25etRXR1r-kILflqmvxhfoj6MUn2bf1FQZTEjOsu5lToKgBdRXIJnqHJbywy80l6j-RrRt2vQRYsp2d_pvY_MX-AIey6BQg-IK6w_BAtJTWft9MYzEoRKNg?purpose=inline)

**Facility Energy Assessment & Flexibility Discovery**

Best fit for FlexCore

For organizations, facilities, aggregators and energy operators.

[Ghidul Programului pentru baterii, finalizat și publicat în Monitorul Oficial după analizarea a peste 200 de puncte de vedere - Ministerul Mediului, Apelor și Pădurilor](https://images.openai.com/static-rsc-4/4oEX-r7sDasXa5y4w_NAvbfcZSxazMupf2hsyhP0emjodE_roilMC-f8eP0IZ9lEfloIIQgqoAlqTc6tr8IrosM_jYs6FeQjxgfJ405y3cUpAXDEbdj9JMTHPntkEFsmkyRd4nU2HrqZ7iFWCW07j-CIFM_mX1pJW9Z3Kd-4gQc?purpose=inline)

**Household Solar & Storage Feasibility Planner**

Best public-facing opportunity

For homeowners deciding whether and how to install solar and batteries.

The distinction is that one strengthens FlexCore as an enterprise infrastructure platform, while the other makes its wider energy ecosystem useful to ordinary households.

Let's look at the actual evidence.

## **1. Is there enough market demand for household solar planning?**

Yes. There is strong evidence of a growing market, but we need to distinguish market growth from demand for another software calculator.

The IEA's *Renewables 2026* forecast projects an additional 1,530 GW of distributed solar PV capacity between 2026 and 2031. This includes residential, commercial, industrial and off-grid applications; it is not a residential-only forecast.&#x20;

[image](https://www.google.com/s2/favicons?domain=https://www.iea.org\&sz=32)

IEA+1



[Photovoltaic Systems in Greece for Home, Business & Full Autonomy](https://images.openai.com/static-rsc-4/L5Yzrz5ggsW5lu8pPU2uVlVZOcI-1sTOhFa9fs3iBA6S3WXbtm9rV4cYlOX_eFWUsgihJfq8QMSR7hEHSKrPOM0_ey7kUWE6Snbm3rLWt626nrcCBqTpCn4LRXzgW6TU2yQP-_psOXY5slVL7R_AfkZF49wtp-avSFU77odrTD4?purpose=inline)

[Autoconsommation, autoproduction électrique : les différences](https://images.openai.com/static-rsc-4/vdHbCr8m_Rat0CoywAlwoUVAtyAhXadxG-jzUG_SLrSNvyQBM3pGKJcgFJWCcTzy4yYH99eeJipFDhPYWTMW6dJPDASnngdHJwW2hoIcmhWrIUKjga80uCe_G2BixFEdqNmC8AkgysK8s7OyD4XHgxroGVF2AW_TktyaXKF5d-s?purpose=inline)

[Solar Battery Storage Sunshine Coast: Is It Worth the Investment? | Keystones Energy Solutions](https://images.openai.com/static-rsc-4/QhZf71QBPx_pjyCQk2Q36aANw8em--43z1PmCgTgbgZ6GGu1Sx9VoXusMKDFRYy9lv_QBJYgF-nrJ2uoN943mjuH2zWkO7kdoHqws-9uLDY4HN3V0fyRx1BKHFe3OObi43GHCPLJvdyRq_0-Fh_3XaexBFXL3w2QARoL2SsCGes?purpose=inline)

9

There is also more specific evidence from India.

A June 2026 study by the Council on Energy, Environment and Water (CEEW) surveyed 17,094 households across 22 states and union territories. Its findings are particularly relevant:

| Research finding                                                                | Result              |
| ------------------------------------------------------------------------------- | ------------------- |
| Households solarised by May 2026                                                | More than 4 million |
| Growth rate reported for residential rooftop solar                              | 85% CAGR            |
| Interested households not initiating the process                                | 71%                 |
| Interested non-initiators who did not know how to begin                         | 68%                 |
| Initiating households experiencing intermediate-stage obstacles                 | 59%                 |
| Households perceiving solar as expensive that were unaware of financing options | 73%                 |

[image](https://www.google.com/s2/favicons?domain=https://www.ceew.in\&sz=32)

ceew\.in+2

These are India-specific findings, not global adoption statistics. But they reveal an important problem.

**The main obstacle isn't necessarily calculating how many solar panels someone needs. It's helping them make a decision and move from interest to installation.**

Households need to understand:

- Whether their roof is suitable.
- How much solar capacity they actually need.
- Whether they need a battery.
- How much installation will cost.
- What financial benefits they can realistically expect.
- What government incentives are available.
- Whether their utility supports net metering or net billing.
- What steps they must complete before installation.
- How to evaluate installers and equipment proposals.

This is considerably more useful than a panel-size calculator.

## **2. Who already provides household solar planning?**

This is where I want to be especially careful. We should not build something that already exists simply because its functionality sounds attractive.

Here are the important existing products and tools.

[What is PVWatts?](https://images.openai.com/static-rsc-4/tyEMoIUMqTuxp5FvjNK1zYl0ef6xiKRcDYncGuo4_tNzs1xJTu1rVThZZMk5okLjVkGe05QUGEKVsdv9NOxZPV773F_kj3mi6oe2IIOQGm706Me4UZh6_lA_YZILOz2T5L-72xykqWuCwlyvWWg5Hhz0HEX3pBP6tcXSdtRd_z4?purpose=inline)

**PVWatts**

Free public tool

Estimates electricity production from a proposed grid-connected solar system using location and system inputs.

It already provides a developer API and uses internationally available solar-resource datasets where coverage permits.

[PVWatts calculator](https://pvwatts.nlr.gov/?utm_source=chatgpt.com) · [API documentation](https://developer.nlr.gov/docs/solar/pvwatts/?utm_source=chatgpt.com)

[Introduction](https://images.openai.com/static-rsc-4/z_iVoK5o2U38e599Cn3-LKeujQwbh3mdANXLcrCFoaBhdP5b0kY69bWO9LB2CtmWDzDrJhd_kn9S2r0ayroCxp9CYS7jdH_bq-0tFYcHUh2x4D9ocL2_vJBY3NhxLKL_yve-lOKI8jV8XAt2I0TPr_gnbH16YOrOom0CIVkmhBA?purpose=inline)

**System Advisor Model (SAM)**

Free software

More detailed technical and financial modeling of residential and commercial solar, battery storage, project economics and electricity costs.

It is particularly important because it already handles work that would otherwise require us to develop our own detailed modeling infrastructure.

[SAM website](https://sam.nlr.gov/?utm_source=chatgpt.com) · [Financial models](https://sam.nlr.gov/financial-models?utm_source=chatgpt.com)

[REopt Lite Evolves To Address User Priorities | NLR](https://images.openai.com/static-rsc-4/Zh8XIiDXjXwx3r63rxuIdBFz8dgprxRisdDPiVETZRV77UAsOlg0vVs-GK-9xVdEC4kVXKmlbDxAzLZR5ON9hQoEIkbpdWjU7iOZCbXhBrndPxI4zzqWKh7SjUVJ9rnTCN3w91V7Lv_nm0CgQNbaWJYVm67ubsCgdfGFzwp7oZk?purpose=inline)

**REopt**

Public tool and open-source code

Optimizes combinations of solar, batteries, conventional generation and controllable loads. It determines system sizing, dispatch and economic performance, including resilience.

In 2026, its web tool also expanded its net-metering, net-billing and sensitivity-analysis capabilities.

[REopt web tool](https://reopt.nlr.gov/tool/?utm_source=chatgpt.com) · [REopt source code](https://github.com/NatLabRockies/REopt.jl?utm_source=chatgpt.com)

[Module 6: Design with Nearmap on OpenSolar 3D - Help Center](https://images.openai.com/static-rsc-4/LHKB5L9WC5D3AQXeDXVrtLxmZ5VojP9LvUcJC-f8gjNahlLM-nTtgMplp1mdhIqkk2Abe93nu48m13j7-aJpJZmNGFieRa5SeehCAPX7NgsfnWnDJjfmbKw03nVbhfu3sct7_dfzOQd05rfqKnCXQg2IDYuS2ecy-bXV0Pi06sM?purpose=inline)

**OpenSolar**

Free core platform

Professional solar design, automated proposals, sales workflows, project management and installation-related functionality.

As of April 16, 2026, its core application remains free, with charges for API access and supported external connectors. The company reports more than 28,000 solar professionals on its platform.

[OpenSolar](https://www.opensolar.com/pro/?utm_source=chatgpt.com)

[image](https://www.google.com/s2/favicons?domain=https://www.opensolar.com\&sz=32)

OpenSolar+1

[Harness the Power of Solar Energy](https://images.openai.com/static-rsc-4/rszXpDd6861vCTbrCrMJlPYdFAFqB_f06cf22r0W-KWkmSjSTasoR7vZxy4OIY5Pi80EQ_JmQ1T94FSXWmzjtiZ2MyaeKXlJFjGNaNI8vjqPU4ugpFTOUv8dhqLndH2iwEBqyOFFpdV-1r0YCOlZyWYZ2_8V5hMkelseUVlA?purpose=inline)

**Aurora Solar**

Advanced 3D roof modeling, shading analysis, solar system design, sales proposals and installer workflows. Its AI capabilities can generate a preliminary home design from an address and utility bill.

This is a more premium professional-software competitor.

[Aurora Solar](https://aurorasolar.com/?utm_source=chatgpt.com)

[image](https://www.google.com/s2/favicons?domain=https://www.aurorasolar.com\&sz=32)

Aurora Solar+2

[EnergySage’s innovative online marketplace simplifies shopping for solar  – Solar Roadmap](https://images.openai.com/static-rsc-4/gsYGH73ZvB0D7dWXyxtSQTLX-UtWjRYgN2Idl44hr7m7QY8f0ACImcviCsF0sjeaZ-rhVYyJgWTmBUDf8KDSXhtqvu__Xbr2eViVYXmGxZ2Z9g-YBmD17JgXZRdN1iSZ27jdhtH5zTh0ER8pOLoQ7gYYFgzT_Wih2fGcJ_DwNzc?purpose=inline)

**EnergySage**

Helps homeowners compare solar quotes from more than 500 pre-screened installers in its network.

This is especially important because it addresses the purchasing decision rather than just energy calculations.

[EnergySage](https://www.energysage.com/?utm_source=chatgpt.com)

[image](https://www.google.com/s2/favicons?domain=https://www.energysage.com\&sz=32)

EnergySage+1

### **What this competition tells us**

There are three different layers of solar planning:

| Layer                                                            | Market situation                                                              |
| ---------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| Estimate solar production                                        | Already well served by free tools                                             |
| Optimize solar and battery sizing                                | Already well served by sophisticated free tools                               |
| Professional 3D system design, sales and installation workflows  | Established vendors with mature products                                      |
| Independent household decision support and installation guidance | A potentially more interesting opportunity, especially in underserved regions |

I would not build a competitor to SAM, REopt, OpenSolar or Aurora.

We would spend a significant amount of time reproducing capabilities already available to the public and industry.

There is a better product opportunity between technical calculations and the homeowner's actual decision.

## **3. What should the household solar application actually do?**

I would define it as a **Household Solar & Storage Feasibility Planner**, not a home energy-audit product.

[Aerial view of modern new  house (with rooftop solar) and garden amongst trees](https://images.openai.com/static-rsc-4/4a0jvTbAAII885OT7ft38Q7-6Y41skpUt284_3GWl9NdVPaj5_vH9vkq7lfjGduUl-9ovQSRWGkOSr3j4OVrY1SAYWZ-CKUddtehORdSEKHRyOF-32O5g88dO0O-EGs8Tcrj8YEpAMMif7cnlnCFod3ti0sAbhMi1p2P6hSIyLf_Q7IFvLO_7gRg58xHI9nE?purpose=inline)

[Home Battery Backup Cost in Texas: 2026 Pricing Breakdown](https://images.openai.com/static-rsc-4/UxsGTQvqMm0ysUGM_ymVDx4NXFwPUkUAKiZ5PLMB4P1Pnenddh7_RkBzGCCJQk_1KiCnvwgqJB2AMn_U6c4wIK46jwD-7f1YT8sMaoJkeLrGWlaxxh0CSx-xwM4u2WhkztRWGh_GEeV3NUkfHfON-MhVYC6s7sjmxlMGmUTouUQ?purpose=inline)

[EDF OA solaire : compte et facturation 2026](https://images.openai.com/static-rsc-4/AKbVDJSr3i87-XpzRnKDrSxCePiA5uT3QuwU6KmCYZ5dHgS1gbROVKoXbcH9-iW_CfnZJZqJ1Fv-uHQf4Anw3dmwS7Kyck8bG9eCttke-W6nB1GkUrxlDX03RxOQXw58MXp_A4Xi76WeCZOprixBRAf6qcSEEIF-Gx2G8FFLZGU?purpose=inline)

9

Its purpose would be to answer one main question:

*Given my electricity consumption, location, roof and financial situation, what is the most sensible solar and storage configuration for my household?*

The application should have five main capabilities.

**1. Household energy profile**

Users provide electricity bills, household location, electricity consumption and relevant appliance information.

It identifies monthly consumption patterns and, when interval data is available, hourly load patterns.

It should not pretend monthly bills provide precise hourly load behavior.

**2. Rooftop solar feasibility**

The user provides their address or location, roof orientation, available area and obstruction information.

Where reliable geospatial data exists, the application can estimate roof geometry and shading. Otherwise, it should use declared inputs and explicitly mark the estimate as less certain.

Google's Solar API can supply building-specific roof and solar-resource insights, but availability and image quality vary geographically. Its usage is also billed on a pay-as-you-go basis.&#x20;

[image](https://www.google.com/s2/favicons?domain=https://developers.google.com\&sz=32)

developers.google.com+2



**3. Solar and battery configuration comparison**

This should be a particularly useful part.

Rather than immediately telling a household to buy a 5 kW system, compare alternative configurations.

For example:

| Scenario             | What it answers                                                 |
| -------------------- | --------------------------------------------------------------- |
| Grid only            | What happens if the household does nothing?                     |
| Solar only           | How much of the household's consumption can solar cover?        |
| Solar + battery      | How does storage affect self-consumption, costs and resilience? |
| Battery only         | Is storage useful without rooftop generation?                   |
| Smaller solar system | Is a lower initial investment more sensible?                    |

The application should determine appropriate configurations from the actual load profile, technical limits, local electricity rates and financial assumptions.

**4. Financial feasibility**

This is where I would invest considerable thought.

Compare:

- Installation cost.
- Annual generation.
- Self-consumed solar energy.
- Exported electricity.
- Electricity bill savings.
- Battery replacement assumptions.
- Financing and interest costs.
- Maintenance.
- Payback period.
- Net present value.
- Sensitivity to electricity prices and equipment costs.

An important detail from PVWatts: its reported energy value effectively assumes net metering. Where exported electricity receives a lower price than imported electricity, the value can be overstated.&#x20;

[image](https://www.google.com/s2/favicons?domain=https://pvwatts.nlr.gov\&sz=32)

pvwatts.nlr.gov



Therefore, a trustworthy planner must use the actual local tariff and export-compensation rules rather than a universal electricity-price assumption.

**5. A decision and implementation report**

The final output should tell the homeowner:

- Whether solar makes economic sense.
- Which configuration is most appropriate.
- What assumptions drive the result.
- What information is still missing.
- What financial risks exist.
- Whether a professional site inspection is necessary.
- What the household needs to do to proceed.

It should be comfortable recommending that a household *not* install solar when the economics or physical conditions do not support it.

That independence could be a meaningful differentiator.

## **4. What about the organizational energy-assessment market?**

Now let's compare the other opportunity.

[Electrical Site Audits | Interstates](https://images.openai.com/static-rsc-4/QEqACE4LQTtjKqaP5670ptzp2RKtsH5BwhUQ8yH4pV6ZLNlnxNl9EdQY4KcCQ8U8JIQmaKkHYT77MIEdrpM68b5HYQJ7E4M42msXGk9K-ONe_JWbz0k1sG652fmnQsiyHdKlERJjlXCsXX3k60QpAUTY5_tdkMQOqDAuwdXABS4?purpose=inline)

[Optimize HVAC Performance with Advanced Chiller Plant Automation | Automated Logic](https://images.openai.com/static-rsc-4/iHJsb5RLj_b9PnBrFJLwnlSBkz86i3SMdF-ytXLSREzGtRiHBqaLCX-3ZJnPHlVwxqJzqDlo2mc8eRctNR_AeT6Ym1fpoBLNIXn2Pf4dwGdSVhfO3kujCwavRK9D-1zmbI6Z7VCtcdMqP9a9eelXSMqp68Q0BMXCkbCxGcFCigY?purpose=inline)

[商业太阳能电池储能案例研究 - Intone Power](https://images.openai.com/static-rsc-4/UaS7QJkUk0C-9FKaAiIsPMqA6YN4bfM7RoXcLHlPkhVdCE_hzlBD25etRXR1r-kILflqmvxhfoj6MUn2bf1FQZTEjOsu5lToKgBdRXIJnqHJbywy80l6j-RrRt2vQRYsp2d_pvY_MX-AIey6BQg-IK6w_BAtJTWft9MYzEoRKNg?purpose=inline)

11

The organizational market is not empty either.

The US Department of Energy offers 50001 Ready, a no-cost structured energy-management program. It also provides an Audit Template based on ASHRAE Standard 211. BuildingSync offers a standardized data-exchange format for commercial building audits.&#x20;

[image](https://www.google.com/s2/favicons?domain=https://www.energy.gov\&sz=32)

Department of Energy+2



REopt already addresses an important portion of the facility-planning problem, including optimal system sizing and dispatch for solar, batteries and other distributed energy resources.&#x20;

[image](https://www.google.com/s2/favicons?domain=https://reopt.nlr.gov\&sz=32)

NLR+1



Consequently, I would not build a full replacement for commercial energy-audit software either.

But organizational assessment has a stronger connection to FlexCore than a normal energy audit.

The critical output is not simply a facility's annual consumption or energy-efficiency report.

It is the facility's **qualified energy-resource inventory**.

Consider a factory that has:

- Solar generation.
- A battery.
- Industrial refrigeration.
- HVAC systems.
- EV charging.
- Controllable industrial processes.

A standard assessment identifies consumption and improvement opportunities.

A FlexCore assessment should additionally establish which of these resources can participate in coordinated energy operation, their constraints, and what evidence supports the estimated flexibility.

That becomes directly useful to an aggregator or multi-site operator.

I would therefore revise our earlier feature definition slightly:

Recommended FlexCore extension

# **Facility DER Assessment & Flexibility Qualification**

An assessment and onboarding capability that produces a structured, evidence-backed representation of a facility's energy resources, operational constraints, efficiency opportunities and available flexibility.

It should support established audit standards and BuildingSync interoperability, but its defining output should be an asset and flexibility inventory for FlexCore.

This is a much stronger extension than simply building an energy-audit report generator.

## **5. Direct comparison: which is actually better?**

Here is my strategic comparison. These are qualitative product judgments, not measured market-size scores.

| Criterion                                     | Facility assessment                           | Household solar planner                                         |
| --------------------------------------------- | --------------------------------------------- | --------------------------------------------------------------- |
| Fit with FlexCore's current product           | Excellent                                     | Limited in the core platform                                    |
| Direct connection to existing DER models      | Excellent                                     | Partial                                                         |
| Public accessibility                          | Limited                                       | Excellent                                                       |
| Potential user reach                          | Organizations                                 | Households                                                      |
| Existing competition                          | Established audit and energy-management tools | Many free calculators and mature platforms                      |
| Differentiation opportunity                   | Flexibility discovery and asset onboarding    | Independent localized decision guidance                         |
| Technical complexity                          | High                                          | Medium for a limited planner; high for professional roof design |
| Commercial opportunity                        | Enterprise contracts and integration          | Less direct; likely partnerships, referrals or sponsorship      |
| Importance of local regulations               | High                                          | Very high                                                       |
| Long-term relationship with VPP orchestration | Direct                                        | Possible through aggregated residential DERs                    |

There is another important consideration: user frequency.

A homeowner may use a solar planner a handful of times before deciding whether to install a system. A facility operator or aggregator could use assessment, asset inventory and monitoring capabilities continuously across many sites.

That affects the product's long-term value and monetization.

### **My ranking**

**1. Facility DER Assessment & Flexibility Qualification**

First

Best extension to FlexCore. It makes the existing platform more useful without changing its product identity.

**2. Household Solar & Storage Feasibility Planner**

Best opportunity for a separate public-facing application within the wider energy product ecosystem. It has genuine public utility, but it needs a sharper proposition than another calculator.

**3. Generic Household Energy Audit**

I would not prioritize this. Its scope is broad, building-specific, and professionally demanding. It also has less direct connection to FlexCore.

## **6. An important geographic consideration**

We cannot build a credible global household planner by assuming every country has the same energy economics.

For example, Nepal already has:

- An AEPC and GIZ guideline for grid-connected rooftop solar covering residential and C&I properties.
- An official Excel-based rooftop solar sizing tool.
- An Excel-based financial analysis tool.
- NEA net-metering provisions.&#x20;

  [image](https://www.google.com/s2/favicons?domain=https://www.aepc.gov.np\&sz=32)

  aepc.gov.np+2



The official AEPC application documentation also demonstrates the importance of the local installation process: its specified documentation includes 12 months of electricity bills and roof photographs, and its particular net-metering application route specifies minimum annual self-consumption and an approved detailed feasibility study.&#x20;

[image](https://www.google.com/s2/favicons?domain=https://www.aepc.gov.np\&sz=32)

aepc.gov.np



This tells us something important: even where the technical calculation exists, the workflow around it matters.

A household product could improve access to these processes, but it would need to reflect the actual national requirements.

India presents a different opportunity, with a large documented household adoption pipeline and government-supported rooftop-solar program. The US has different tariffs, financing and incentive structures.

We should therefore design for multiple regional configurations eventually, but **launch with one explicitly selected geographic market**.

I would not silently choose Nepal, India or the US as the first market without validating where we intend to find actual users.

## **7. What I would actually add to FlexCore**

Here is the roadmap I recommend now.

**FlexCore product roadmap additions**

1. **F-16 — Facility DER Assessment & Flexibility Qualification**

   Proposed post-v1

   Organizational energy assessment, resource inventory, operational constraints, flexibility qualification and integration with existing asset onboarding.
2. **F-17 — Residential Solar & Storage Feasibility Planner**

   Research candidate

   A separate homeowner-facing application for assessing roof solar potential, comparing PV and battery configurations, evaluating costs and helping people make installation decisions.

   It should not be another workspace in the enterprise operator console.
3. **Later — Regional energy planning packs**

   Region-specific tariffs, net-metering/net-billing rules, equipment costs, incentives, financing assumptions and installation workflows. Each pack needs a source, jurisdiction and validity date.

### **Architectural boundary**

I would keep both capabilities in the FlexCore ecosystem, but not combine their user interfaces or force them into one workflow.

```
Neryva Energy Ecosystem
│
└── FlexCore
    │
    ├── Core VPP / DER Orchestration
    │   ├── Asset Registry
    │   ├── Telemetry
    │   ├── Flexibility
    │   ├── Scheduling
    │   ├── Optimization
    │   └── Grid Validation
    │
    ├── Facility Assessment
    │   └── Facility DER & Flexibility Onboarding
    │
    └── Companion Applications
        └── Household Solar Planner
            ├── Solar Feasibility
            ├── Battery Scenarios
            ├── Financial Comparison
            └── Installation Readiness
```

The two applications can reuse relevant energy-resource contracts and calculation infrastructure, but their requirements must remain separate.

For solar calculations, I would first benchmark against PVWatts, SAM and REopt rather than implement another numerical optimization engine. For building-audit exchange, I would evaluate BuildingSync.

There is no reason to duplicate mature scientific work just to claim that we implemented it ourselves.

## **8. What I would not do yet**

One important matter about the current repository: its README still records the code as written but unrun, with its first full build and test pass outstanding and the core license decision unresolved.

[FlexCore repository — current release and verification status](https://github.com/Luke23-45/energy-flexcore?utm_source=chatgpt.com)

Neither proposed feature should be allowed to delay those gates.

I would also require a small validation exercise before committing to the household application:

1. Build a working prototype using a single selected country's real tariff and installation assumptions.
2. Compare its technical and financial outputs against PVWatts/SAM or REopt for identical inputs.
3. Test it with actual homeowners who are considering solar, not just engineers.
4. Find out whether they need an estimate, a configuration comparison, or help navigating the installation process.
5. Verify that the output is sufficiently accurate and useful to justify maintaining the regional data.

That would tell us whether we have something people actually need rather than an attractive demonstration.

# **Final recommendation**

I would retain the two opportunities, but make different strategic commitments to them.

**For FlexCore itself: prioritize organizational facility assessment and flexibility discovery.** This strengthens the existing platform, directly supports its intended operators and gives an assessment a meaningful purpose beyond reporting energy consumption.

**For ordinary people: pursue household solar and battery planning as a companion application.** I think this is a better public-facing product than a general household energy audit. But it must help people decide whether, what and how to install—not simply tell them how many panels to buy.

There is an important reason I would not discard the household direction: the CEEW research identifies substantial friction between household interest and actual solar adoption. Making the decision and installation journey easier is a real problem, not an invented product opportunity.

The key is not to compete with NREL on solar mathematics or with OpenSolar on professional installer software. It is to provide a trustworthy, accessible and regionally accurate decision tool.

**I would not launch both simultaneously.** Keep F-16 as the proposed next FlexCore extension, and retain F-17 as a research candidate until the household prototype demonstrates real demand.

One last thing: we should resist the temptation to make FlexCore do everything related to energy. Its value comes from a coherent platform. We can serve more people through carefully separated applications without turning its core into an unrelated collection of products.

That is the direction I would back.
