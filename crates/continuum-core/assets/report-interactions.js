(async () => {
  const dark = window.matchMedia('(prefers-color-scheme: dark)').matches;
  mermaid.initialize({startOnLoad:false, securityLevel:'strict', htmlLabels:false, theme:'base',
    themeVariables:{primaryColor:dark?'#20372f':'#edf6f1',primaryTextColor:dark?'#edf4ef':'#172019',primaryBorderColor:'#568c78',lineColor:dark?'#aab8ae':'#5e6a62',edgeLabelBackground:dark?'#18241e':'#f3f7f4',fontFamily:'system-ui, sans-serif'},
    flowchart:{htmlLabels:false,useMaxWidth:false,curve:'basis',nodeSpacing:42,rankSpacing:70,padding:18}});
  let index=0;
  for(const figure of document.querySelectorAll('figure[data-renderer]')) {
    const source=figure.querySelector('.mermaid-source').textContent;
    const viewport=figure.querySelector('.diagram-scroll'), status=figure.querySelector('.diagram-status');
    const select=document.createElement('select');
    select.setAttribute('aria-label','Diagram direction');
    for(const [value,label] of [['TD','Vertical'],['LR','Horizontal']]) {const option=document.createElement('option');option.value=value;option.textContent=label;select.append(option);}
    select.value=/flowchart TD/.test(source)?'TD':'LR';
    figure.querySelector('.diagram-tools').append(select);
    let width=800,height=400,zoom=1,svg,drag,rendering=false;
    const apply=()=>{if(!svg)return;svg.style.width=(width*zoom)+'px';svg.style.height=(height*zoom)+'px';status.textContent=Math.round(zoom*100)+'%';};
    const fittedZoom=()=>Math.max(.08,Math.min(1.5,(viewport.clientWidth-32)/width,(viewport.clientHeight-32)/height));
    const fit=()=>{zoom=fittedZoom();apply();viewport.scrollTo(0,0);};
    const render=async()=>{
      if(rendering)return;rendering=true;select.disabled=true;
      try {
        const rendered=await mermaid.render('report-diagram-'+index++,source.replace(/^flowchart (TD|LR)/,'flowchart '+select.value));
        viewport.innerHTML=rendered.svg;svg=viewport.querySelector('svg');
        width=svg.viewBox.baseVal.width||800;height=svg.viewBox.baseVal.height||400;svg.style.maxWidth='none';
        // Start at readable text size. Fit remains an explicit whole-map overview.
        zoom=Math.max(.8,Math.min(1,fittedZoom()));apply();viewport.scrollTo(0,0);
      } catch {status.textContent='Simplified map · text explanation below';}
      finally {rendering=false;select.disabled=false;}
    };
    select.addEventListener('change',render);
    figure.querySelector('[data-diagram="fit"]').addEventListener('click',fit);
    for(const [action,factor] of [['in',1.25],['out',.8]]) figure.querySelector('[data-diagram="'+action+'"]').addEventListener('click',()=>{zoom=Math.min(3,Math.max(.08,zoom*factor));apply();});
    figure.querySelector('[data-diagram="full"]').addEventListener('click',async()=>{try{if(document.fullscreenElement)await document.exitFullscreen();else await figure.requestFullscreen();}catch{status.textContent='Fullscreen unavailable; use zoom';}});
    viewport.addEventListener('pointerdown',e=>{if(e.button!==0)return;drag=[e.clientX,e.clientY,viewport.scrollLeft,viewport.scrollTop];viewport.setPointerCapture(e.pointerId);});
    viewport.addEventListener('pointermove',e=>{if(drag){viewport.scrollLeft=drag[2]-e.clientX+drag[0];viewport.scrollTop=drag[3]-e.clientY+drag[1];}});
    const stop=()=>{drag=null;};viewport.addEventListener('pointerup',stop);viewport.addEventListener('pointercancel',stop);
    await render();
  }
})();
