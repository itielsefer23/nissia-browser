//! Dismiss action: close cookie/consent banners (CMPs) and remove blocking
//! fixed overlays so the page becomes readable. Runs entirely in the page (one
//! cheap JS round-trip); handles OneTrust/Didomi/Sourcepoint/Quantcast, generic
//! accept buttons by text (multi-language), iframes, and large fixed overlays /
//! ad containers (outbrain/taboola).

use nissia_cdp::commands::RuntimeEvaluate;
use nissia_cdp::CdpTransport;

const DISMISS_JS: &str = r#"(function(){var clicked=[];var sels=['#onetrust-accept-btn-handler','#didomi-notice-agree-button','.fc-cta-consent','.fc-button-consent','#sp-cc-accept','.qc-cmp2-summary-buttons button','button[mode=primary]','[data-testid=accept-button]','[aria-label*=accept i]','[aria-label*=aceptar i]','.cc-allow','.cc-dismiss','#cookie-accept','.cookie-accept'];function docs(){var a=[document];try{[].forEach.call(document.querySelectorAll('iframe'),function(f){try{if(f.contentDocument)a.push(f.contentDocument);}catch(e){}});}catch(e){}return a;}var rx=/^(aceptar|aceptar todo|aceptar y cerrar|acepto|accept|accept all|i accept|aceitar|aceitar todos|agree|i agree|consent|got it|entendido|de acuerdo|allow all|permitir|continuar|ok)$/i;docs().forEach(function(d){sels.forEach(function(se){try{var b=d.querySelector(se);if(b){b.click();clicked.push(se);}}catch(e){}});try{var bs=[].slice.call(d.querySelectorAll('button,[role=button],a'));for(var i=0;i<bs.length;i++){var t=((bs[i].innerText||bs[i].textContent||'')+'').trim();if(t.length<25&&rx.test(t)){bs[i].click();clicked.push(t);break;}}}catch(e){}});var removed=0;var vw=window.innerWidth;var vh=window.innerHeight;[].slice.call(document.querySelectorAll('div,section,aside,iframe,dialog')).forEach(function(e){try{var s=getComputedStyle(e);if(s.display==='none')return;var pos=s.position;var z=parseInt(s.zIndex)||0;var r=e.getBoundingClientRect();var big=r.height>60&&r.width>200;var covers=r.width>=vw*0.6&&r.height>=vh*0.5;var id=((e.id||'')+' '+(e.className||'')+'').toLowerCase();var bn=/cookie|consent|gdpr|cmp|banner|modal|overlay|popup|paywall|newsletter|subscrib|interstitial|backdrop|sp_message|qc-cmp|didomi|onetrust|truste|outbrain|taboola|sponsor|promoted/.test(id);var fixedish=(pos==='fixed'||pos==='sticky');if((fixedish&&big&&z>=40)||(bn&&big)||((fixedish||pos==='absolute')&&covers&&z>=10)){e.remove();removed++;}}catch(_){}});try{document.documentElement.style.overflow='auto';if(document.body){document.body.style.overflow='auto';}}catch(e){}return JSON.stringify({accepted: clicked.length? clicked.slice(0,3).join(', '): null, removed: removed});})()"#;

/// Run the dismiss routine once. Returns the raw JSON result string
/// (`{"accepted": "...", "removed": N}`).
pub async fn execute(transport: &CdpTransport) -> Result<String, nissia_cdp::CdpTransportError> {
    let result = transport
        .send(&RuntimeEvaluate {
            expression: DISMISS_JS.to_string(),
            return_by_value: Some(true),
            await_promise: Some(true),
            context_id: None,
        })
        .await?;
    let val = result.result.value.unwrap_or(serde_json::Value::Null);
    Ok(match val {
        serde_json::Value::String(s) => s,
        other => other.to_string(),
    })
}
