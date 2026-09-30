use frontlane_serp::core::types::ResultType;
use frontlane_serp::engines::baidu::features::extract_baidu_features;
use frontlane_serp::engines::bing::features::extract_bing_features;
use frontlane_serp::engines::duckduckgo::features::extract_duckduckgo_features;
use frontlane_serp::engines::ecosia::features::extract_ecosia_features;
use frontlane_serp::engines::google::features::extract_google_features;
use frontlane_serp::engines::yandex::features::extract_yandex_features;
use scraper::Html;

#[test]
fn test_google_serp_features_extraction() {
    let html_str = r#"
        <html>
        <body>
            <div data-mcpr="1">
                <div data-subtree="aimc">
                    <h2>AI Overview</h2>
                    <p>Antigravity is an agentic AI coding assistant.</p>
                    <a href="https://example.com/ai-source">Source Link</a>
                </div>
            </div>
            <div data-initq="1">
                <div class="related-question-pair" data-q="What is OpenSERP?">
                    <a href="https://example.com/openserp">What is OpenSERP?</a>
                </div>
            </div>
            <div jsname="yEVEwb" role="navigation">
                <a href="/search?q=rust+serp+scraper">rust serp scraper</a>
            </div>
        </body>
        </html>
    "#;
    let doc = Html::parse_document(html_str);
    let features = extract_google_features(&doc);

    assert!(!features.is_empty(), "Google should extract features");
    assert!(features
        .iter()
        .any(|f| f.feature_type == ResultType::AiSummary));
    assert!(features
        .iter()
        .any(|f| f.feature_type == ResultType::PeopleAlsoAsk));
    assert!(features
        .iter()
        .any(|f| f.feature_type == ResultType::RelatedSearches));
}

#[test]
fn test_bing_serp_features_extraction() {
    let html_str = r#"
        <html>
        <body>
            <li class="b_ans">
                <div class="b_focusLabel">Calculation</div>
                <div class="b_focusTextLarge">42</div>
            </li>
            <div class="b_rrsr">
                <div class="df_qntext">How does Bing rank websites?</div>
            </div>
            <div id="inline_rs">
                <li class="rslist"><a href="https://www.bing.com/search?q=bing+api">bing api</a></li>
            </div>
        </body>
        </html>
    "#;
    let doc = Html::parse_document(html_str);
    let features = extract_bing_features(&doc);

    assert!(!features.is_empty(), "Bing should extract features");
    assert!(features
        .iter()
        .any(|f| f.feature_type == ResultType::AnswerBox));
    assert!(features
        .iter()
        .any(|f| f.feature_type == ResultType::RelatedQuestions));
    assert!(features
        .iter()
        .any(|f| f.feature_type == ResultType::RelatedSearches));
}

#[test]
fn test_duckduckgo_serp_features_extraction() {
    let html_str = r#"
        <html>
        <body>
            <section data-testid="duckassist">
                <h2 data-testid="duckassist-title">DuckAssist Answer</h2>
                <div data-testid="duckassist-expanded-answer-content">
                    DuckDuckGo privacy features keep searches private.
                </div>
            </section>
            <div id="zero_click_wrapper">
                <div class="c-base__title">Instant Answer</div>
                <div class="js-about-item-abstr">Direct quick response facts.</div>
            </div>
            <div data-testid="related-searches">
                <a href="/?q=privacy+tools">privacy tools</a>
            </div>
        </body>
        </html>
    "#;
    let doc = Html::parse_document(html_str);
    let features = extract_duckduckgo_features(&doc);

    assert!(!features.is_empty(), "DuckDuckGo should extract features");
    assert!(features
        .iter()
        .any(|f| f.feature_type == ResultType::AiSummary));
    assert!(features
        .iter()
        .any(|f| f.feature_type == ResultType::AnswerBox));
    assert!(features
        .iter()
        .any(|f| f.feature_type == ResultType::RelatedSearches));
}

#[test]
fn test_yandex_serp_features_extraction() {
    let html_str = r#"
        <html>
        <body>
            <li data-fast-name="neuro_answer">
                <h2 class="FuturisTitle">Нейро ответ</h2>
                <div class="FuturisText">Ответ нейросети Yandex с источниками.</div>
                <a class="FuturisSource" href="https://example.com/source">Источник</a>
            </li>
            <li data-fast-name="fact">
                <h2 class="FactAnswer-Title">Столица Франции</h2>
                <div class="FactAnswer-Text">Париж</div>
            </li>
            <div class="RelatedSearches">
                <div class="Related-Item"><a href="/search?text=yandex+weather">погода яндекс</a></div>
            </div>
        </body>
        </html>
    "#;
    let doc = Html::parse_document(html_str);
    let features = extract_yandex_features(&doc);

    assert!(!features.is_empty(), "Yandex should extract features");
    assert!(features
        .iter()
        .any(|f| f.feature_type == ResultType::AiSummary));
    assert!(features
        .iter()
        .any(|f| f.feature_type == ResultType::AnswerBox));
    assert!(features
        .iter()
        .any(|f| f.feature_type == ResultType::RelatedSearches));
}

#[test]
fn test_baidu_serp_features_extraction() {
    let html_str = r#"
        <html>
        <body>
            <div tpl="ai_chat" class="ai-answer">
                <h3 class="c-title">AI智能回答</h3>
                <div class="ai-answer-text">百度文心智能摘要内容。</div>
            </div>
            <div class="op_exactqa_s_answer">
                <h3 class="op_exactqa_title">北京时间</h3>
                <div class="op_exactqa_detail">12:00:00</div>
            </div>
            <div tpl="app/rs" id="rs">
                <table>
                    <tr><td><a href="https://www.baidu.com/s?wd=rust">rust教程</a></td></tr>
                </table>
            </div>
        </body>
        </html>
    "#;
    let doc = Html::parse_document(html_str);
    let features = extract_baidu_features(&doc);

    assert!(!features.is_empty(), "Baidu should extract features");
    assert!(features
        .iter()
        .any(|f| f.feature_type == ResultType::AiSummary));
    assert!(features
        .iter()
        .any(|f| f.feature_type == ResultType::AnswerBox));
    assert!(features
        .iter()
        .any(|f| f.feature_type == ResultType::RelatedSearches));
}

#[test]
fn test_ecosia_serp_features_extraction() {
    let html_str = r#"
        <html>
        <body>
            <div data-test-id="instant-answer">
                <h2 data-test-id="entity-title">Berlin</h2>
                <div data-test-id="entity-description">Capital of Germany.</div>
            </div>
            <div data-test-id="web-related-queries">
                <a data-test-id="related-query" href="https://www.ecosia.org/search?q=germany">germany travel</a>
            </div>
        </body>
        </html>
    "#;
    let doc = Html::parse_document(html_str);
    let features = extract_ecosia_features(&doc);

    assert!(!features.is_empty(), "Ecosia should extract features");
    assert!(features
        .iter()
        .any(|f| f.feature_type == ResultType::AnswerBox));
    assert!(features
        .iter()
        .any(|f| f.feature_type == ResultType::RelatedSearches));
}

#[test]
fn test_google_deep_ai_overview_citations() {
    let html_str = r#"
        <html>
        <body>
            <div data-attrid="wa_overview">
                <div data-subtree="aimc">
                    <h2>AI Overview</h2>
                    <p>Dental implants provide a permanent foundation for replacement teeth.</p>
                    <p>They fuse directly with bone tissue through osseointegration.</p>
                </div>
                <div class="citation-container">
                    <a href="https://www.mayoclinic.org/tests-procedures/dental-implant-surgery/about/pac-20384622">Mayo Clinic Dental Implants</a>
                    <a href="https://www.colgate.com/en-us/oral-health/implants">Colgate Oral Care</a>
                </div>
            </div>
        </body>
        </html>
    "#;
    let doc = Html::parse_document(html_str);
    let features = extract_google_features(&doc);

    let ai_feature = features.iter().find(|f| f.feature_type == ResultType::AiSummary)
        .expect("Should extract AI Overview");

    assert!(ai_feature.text.as_ref().unwrap().contains("Dental implants provide"));
    assert!(ai_feature.text.as_ref().unwrap().contains("osseointegration"));
    assert_eq!(ai_feature.links.len(), 2);
    assert_eq!(ai_feature.items.len(), 2);

    let citation1 = &ai_feature.items[0];
    assert_eq!(citation1.domain.as_deref(), Some("www.mayoclinic.org"));
    assert_eq!(citation1.title.as_deref(), Some("Mayo Clinic Dental Implants"));
}

#[test]
fn test_google_deep_local_pack_extraction() {
    let html_str = r#"
        <html>
        <body>
            <div class="VkpGBb" data-cid="1234567890" data-place-id="ChIJ_test_place_id">
                <div role="heading" class="dbg0pd">Downtown Dental Center</div>
                <div>
                    <span class="yi40Hd" aria-label="4.9 stars">4.9</span>
                    <span class="RDApEe">(128 reviews)</span>
                </div>
                <div class="rllt__details">
                    <div>123 Main St, Los Angeles, CA</div>
                    <div>Open · Closes 6 PM</div>
                </div>
                <a href="https://www.google.com/maps?cid=1234567890&ll=@34.0522,-118.2437">Map & Directions</a>
            </div>
            <div class="VkpGBb" data-cid="9876543210">
                <div class="dbg0pd">Smile Craft Specialists</div>
                <div>
                    <span class="yi40Hd">5.0</span>
                    <span class="RDApEe">(342)</span>
                </div>
                <div class="rllt__details">
                    <div>456 Wilshire Blvd, Beverly Hills, CA</div>
                    <div>Open · Closes 5 PM</div>
                </div>
                <a href="https://www.google.com/maps?cid=9876543210&ll=@34.0689,-118.4053">Directions</a>
            </div>
        </body>
        </html>
    "#;
    let doc = Html::parse_document(html_str);
    let features = extract_google_features(&doc);

    let local_pack = features.iter().find(|f| f.feature_type == ResultType::Local)
        .expect("Should extract Local 3-Pack");

    assert_eq!(local_pack.items.len(), 2);

    let place1 = &local_pack.items[0];
    assert_eq!(place1.title.as_deref(), Some("Downtown Dental Center"));
    assert_eq!(place1.rating, Some(4.9));
    assert_eq!(place1.reviews_count, Some(128));
    assert_eq!(place1.cid.as_deref(), Some("1234567890"));
    assert_eq!(place1.place_id.as_deref(), Some("ChIJ_test_place_id"));
    assert!(place1.address.as_ref().unwrap().contains("123 Main St"));
    assert!(place1.hours.as_ref().unwrap().contains("Closes 6 PM"));

    let coords1 = place1.coordinates.as_ref().unwrap();
    assert!((coords1.lat - 34.0522).abs() < 1e-4);
    assert!((coords1.lng - -118.2437).abs() < 1e-4);

    let place2 = &local_pack.items[1];
    assert_eq!(place2.title.as_deref(), Some("Smile Craft Specialists"));
    assert_eq!(place2.rating, Some(5.0));
    assert_eq!(place2.reviews_count, Some(342));
    assert_eq!(place2.cid.as_deref(), Some("9876543210"));
}
