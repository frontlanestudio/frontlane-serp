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
