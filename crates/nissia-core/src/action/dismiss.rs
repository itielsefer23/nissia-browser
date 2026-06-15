//! Dismiss action: close cookie/consent banners (CMPs) and remove blocking
//! fixed overlays so the page becomes readable. Runs entirely in the page (one
//! cheap JS round-trip); handles OneTrust/Didomi/Sourcepoint/Quantcast, generic
//! accept buttons by text (multi-language), iframes, and large fixed overlays /
//! ad containers (outbrain/taboola).

use nissia_cdp::commands::RuntimeEvaluate;
use nissia_cdp::CdpTransport;

const DISMISS_JS: &str = r#"(function(){var clicked=0,removed=0;function docs(){var a=[document];try{[].forEach.call(document.querySelectorAll('iframe'),function(f){try{if(f.contentDocument)a.push(f.contentDocument);}catch(e){}});}catch(e){}return a;}
var accept=['#onetrust-accept-btn-handler','#didomi-notice-agree-button','.fc-cta-consent','.fc-button-consent','#sp-cc-accept','.qc-cmp2-summary-buttons button','#CybotCookiebotDialogBodyButtonAccept','#CybotCookiebotDialogBodyLevelButtonLevelOptinAllowAll','button[data-testid="uc-accept-all-button"]','.cky-btn-accept','.osano-cm-accept-all','.cc-allow','.cc-dismiss','#cookie-accept','.cookie-accept','[data-cookiebanner=accept_button]','button[mode=primary]','[data-testid=accept-button]','[aria-label*=accept i]','[aria-label*=aceptar i]','[aria-label*=aceitar i]'];
var acceptRx=/^(aceptar|aceptar todo|aceptar todas|aceptar y cerrar|acepto|accept|accept all|i accept|allow all|aceitar|aceitar todos|concordo|agree|i agree|consent|got it|entendido|de acuerdo|permitir|continuar|prosseguir|ok)$/i;
var closeSel=['[aria-label*=close i]','[aria-label*=cerrar i]','[aria-label*=fechar i]','[title*=close i]','[title*=cerrar i]','.modal-close','.modal__close','.close-button','.popup-close','.mfp-close','.fancybox-close','.fancybox-button--close','[data-dismiss=modal]','.dialog-close','.newsletter-close','.mc-closeModal'];
var closeRx=/^(×|✕|✖|x|cerrar|fechar|close|no gracias|no, gracias|ahora no|no thanks|no, thanks|maybe later|mas tarde|más tarde|recusar|rejeitar|rechazar|dispensar|saltar|skip)$/i;
docs().forEach(function(d){accept.forEach(function(se){try{var b=d.querySelector(se);if(b){b.click();clicked++;}}catch(e){}});try{var bs=[].slice.call(d.querySelectorAll('button,[role=button],a,input[type=button],input[type=submit]'));for(var i=0;i<bs.length;i++){var t=((bs[i].innerText||bs[i].value||bs[i].textContent||'')+'').trim();if(t.length<22&&acceptRx.test(t)){bs[i].click();clicked++;break;}}}catch(e){}
closeSel.forEach(function(se){try{[].slice.call(d.querySelectorAll(se)).forEach(function(b){var r=b.getBoundingClientRect();if(r.width>0&&r.width<90&&r.height<90){b.click();clicked++;}});}catch(e){}});
try{var cb=[].slice.call(d.querySelectorAll('button,[role=button],a,span'));for(var j=0;j<cb.length;j++){var ct=((cb[j].innerText||cb[j].textContent||'')+'').trim();if(ct.length<14&&closeRx.test(ct)){var rr=cb[j].getBoundingClientRect();if(rr.width>0&&rr.width<130&&rr.height<90){cb[j].click();clicked++;}}}}catch(e){}});
var vw=window.innerWidth,vh=window.innerHeight;
var adSel='ins.adsbygoogle,iframe[src*=googlesyndication],iframe[src*=doubleclick],iframe[src*=adservice],iframe[id^=google_ads],[id^=div-gpt-ad],[id^=google_ads],[class*=adsbygoogle]';
[].slice.call(document.querySelectorAll(adSel)).forEach(function(e){try{var host=e.closest('div[id],aside,section')||e;var s=getComputedStyle(host);if(s.position==='fixed'||s.position==='sticky'){host.remove();}else{e.remove();}removed++;}catch(_){}});
[].slice.call(document.querySelectorAll('div,section,aside,iframe,dialog')).forEach(function(e){try{var s=getComputedStyle(e);if(s.display==='none')return;var pos=s.position;var z=parseInt(s.zIndex)||0;var r=e.getBoundingClientRect();var big=r.height>60&&r.width>200;var covers=r.width>=vw*0.55&&r.height>=vh*0.45;var id=((e.id||'')+' '+(e.className||'')+'').toLowerCase();var bn=/cookie|consent|gdpr|cmp|modal|overlay|popup|pop-up|paywall|newsletter|subscrib|interstitial|backdrop|lightbox|sp_message|qc-cmp|didomi|onetrust|truste|outbrain|taboola|sponsor|promoted/.test(id);var fixedish=(pos==='fixed'||pos==='sticky');if((fixedish&&big&&z>=40)||(bn&&(fixedish||covers))||((fixedish||pos==='absolute')&&covers&&z>=10)){e.remove();removed++;}}catch(_){}});
try{document.documentElement.style.overflow='auto';if(document.body){document.body.style.overflow='auto';document.body.style.position='static';}}catch(e){}
if(!window.__nzGuard){window.__nzGuard=1;var isBlk=function(e){try{var s=getComputedStyle(e);if(s.display==='none')return false;var p=s.position,z=parseInt(s.zIndex)||0,r=e.getBoundingClientRect();var cov=r.width>=window.innerWidth*0.55&&r.height>=window.innerHeight*0.45;var id=((e.id||'')+' '+(e.className||'')).toLowerCase();var bn=/cookie|consent|gdpr|cmp|modal|overlay|popup|pop-up|interstitial|backdrop|lightbox|paywall|sp_message|qc-cmp|didomi|onetrust/.test(id);var fx=(p==='fixed'||p==='sticky');return (fx&&cov&&z>=10)||(bn&&(fx||cov));}catch(_){return false;}};var ob=new MutationObserver(function(ms){for(var k=0;k<ms.length;k++){var an=ms[k].addedNodes;for(var q=0;q<an.length;q++){var nd=an[q];if(!nd||nd.nodeType!==1)continue;try{if(isBlk(nd)){nd.remove();}else if(nd.querySelectorAll){[].slice.call(nd.querySelectorAll('div,section,aside,dialog')).slice(0,40).forEach(function(c){if(isBlk(c))c.remove();});}}catch(_){}}}try{if(document.body)document.body.style.overflow='auto';}catch(_){}});try{ob.observe(document.documentElement,{childList:true,subtree:true});}catch(_){}}
return JSON.stringify({accepted: clicked||null, removed: removed});})()"#;

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
