const fs=require('fs');
const crypto=require('crypto');
const path=require('path');
const {chromium}=require('../../apps/web/node_modules/@playwright/test');
const ledger=require('./real-repo-source-v7.json');
const sha=s=>crypto.createHash('sha256').update(s).digest('hex');

function compareSummaries(expected,actual,at='summary'){
 const differences=[];
 if(Array.isArray(expected)&&Array.isArray(actual)){
  if(expected.length!==actual.length) differences.push(`${at}.length`);
  for(let i=0;i<Math.min(expected.length,actual.length);i++) differences.push(...compareSummaries(expected[i],actual[i],`${at}[${i}]`));
 }else if(expected&&actual&&typeof expected==='object'&&typeof actual==='object'){
  for(const key of new Set([...Object.keys(expected),...Object.keys(actual)])) differences.push(...compareSummaries(expected[key],actual[key],`${at}.${key}`));
 }else if(expected!==actual) differences.push(at);
 return differences;
}

function comparableSummary(summary,freshSnapshot){
 const result=structuredClone(summary);
 if(freshSnapshot){
  // A fresh CLI run changes analyzedAt; the separate evaluator checks its source and relations.
  delete result.graphSha256;
  delete result.analyzedAt;
  for(const selection of result.selections??[]) delete selection.contextSha256;
 }
 return result;
}

async function run(){
 const freshSnapshot=process.argv.includes('--fresh-snapshot');
 const [graphPath,outDir,origin='https://codebasecanvas.einstein-4s-1110.workers.dev',baselinePath=path.join(__dirname,'real-repo-ui-summary.json')]=process.argv.slice(2).filter(arg=>arg!=='--fresh-snapshot');
 if(!graphPath||!outDir) throw Error('usage: node check-real-repo-ui.cjs GRAPH_PATH OUTPUT_DIR [PUBLIC_ORIGIN] [BASELINE_JSON] [--fresh-snapshot]');
 const repoRoot=path.resolve(__dirname,'../..'),resolvedOutDir=path.resolve(outDir);
 if(resolvedOutDir===repoRoot||resolvedOutDir.startsWith(`${repoRoot}${path.sep}`)) throw Error('output directory must be outside product repository');
 const graph=JSON.parse(fs.readFileSync(graphPath,'utf8'));
 fs.mkdirSync(outDir,{recursive:true});
 const browser=await chromium.launch({headless:true});
 try{
 const context=await browser.newContext({viewport:{width:1440,height:900},serviceWorkers:'block',permissions:['clipboard-read','clipboard-write']});
 const page=await context.newPage();const requests=[],consoleErrors=[];
 page.on('request',r=>requests.push({method:r.method(),url:r.url(),postBytes:r.postData()?.length??0}));
 page.on('console',m=>{if(m.type()==='error')consoleErrors.push(m.text())});
 page.on('pageerror',e=>consoleErrors.push(e.message));
 await page.goto(origin+'/',{waitUntil:'networkidle'});
 const initialRequests=requests.length;
 await page.locator('#graph-file').setInputFiles(graphPath);
 await page.getByRole('heading',{name:'Graph loaded and validated'}).waitFor({timeout:20000});
 await page.locator('.canvas-viewport').waitFor({timeout:20000});
 const analysis=await page.getByRole('region',{name:'Analysis'}).innerText();
 if(!analysis.includes(graph.metadata.analyzedAt)) throw Error('Analysis panel has wrong analyzedAt');
 const canvas=await page.locator('.canvas-viewport').evaluate(el=>({generation:el.getAttribute('data-graph-generation'),layoutReady:el.getAttribute('data-layout-ready'),nodes:el.getAttribute('data-graph-nodes'),edges:el.getAttribute('data-graph-edges')}));
 const result={url:origin+'/',graphSha256:sha(fs.readFileSync(graphPath)),analyzedAt:graph.metadata.analyzedAt,analysis,canvas,selections:[],requestsAfterImport:[],consoleErrors};
 for(const [index,target] of ledger.uiSelection.entries()){
  const item=ledger.items.find(i=>i.family==='declaration'&&i.owner===target.name),id=item.expectedNode.id;
  await page.getByRole('searchbox',{name:'Search symbols'}).fill(target.name);
  await page.getByRole('list',{name:'Search results'}).getByRole('button').filter({hasText:id}).click();
  await page.locator('.canvas-viewport').waitFor();
  const selected=await page.locator('.canvas-viewport').getAttribute('data-selected-nodes');
  const details=page.locator('aside[aria-label="Node details"]');
  await details.getByText('Canonical node ID').click();
  const detailText=await details.innerText();
  await page.evaluate(()=>navigator.clipboard.writeText('ISSUE30_SENTINEL'));
  await details.getByRole('button',{name:`Copy Context for ${target.name}`}).click();
  await details.getByText(`Copied Context for ${target.name}.`).waitFor({timeout:10000});
  const copied=await page.evaluate(()=>navigator.clipboard.readText());
  const unescaped=copied.replaceAll('\\_','_');
  fs.writeFileSync(`${outDir}/${target.name}.md`,copied);
  const snapshot={name:target.name,id,reason:target.reason,selectedMatches:selected===id,detailsContainsId:detailText.includes(id),detailsContainsAnalyzedAt:detailText.includes(graph.metadata.analyzedAt),detailsContainsNodeEvidence:detailText.includes('Node evidence'),detailsContainsEdgeEvidence:detailText.includes('Edge evidence'),detailsHasLocalUnknown:detailText.includes('Related unknown / diagnostics'),detailsHasGraphCallAnalysis:detailText.includes('Graph-wide recorded call analysis'),detailsHasScopedSkipped:detailText.includes('Selected method scope skipped call sites'),contextSha256:sha(copied),contextCodePoints:Array.from(copied).length,contextContainsId:copied.includes(id),contextContainsAnalyzedAt:copied.includes(graph.metadata.analyzedAt),contextHasScope:copied.includes('Analysis scope / unknown'),contextHasTruncation:copied.includes('## Truncation'),contextExcerpt:copied.slice(0,750),detailExcerpt:detailText.slice(0,1200)};
  if(target.name==='ArticleController') {snapshot.moduleInDetails=detailText.includes('ArticleModule');snapshot.routesInDetails=detailText.includes('GET /articles');snapshot.routesInContext=copied.includes('GET /articles');}
  if(target.name==='AuthMiddleware') {snapshot.requestedTokenInDetails=detailText.includes('Requests token')&&detailText.includes('UserService');snapshot.requestedTokenInContext=copied.includes('Requests token')&&copied.includes('UserService');snapshot.unknownInContext=unescaped.includes('unsupported_call_injected_receiver');}
  if(target.name==='ProfileService') {snapshot.unsupportedInDetails=detailText.includes('unsupported_di_decorator_origin');snapshot.unsupportedInContext=unescaped.includes('unsupported_di_decorator_origin');}
  snapshot.contextOmittedDiagnostics=(copied.match(/Relevant diagnostics \(selected node and included methods\): candidates=(\d+); included=(\d+); omitted=(\d+)/)||[]).slice(1).map(Number);
  snapshot.contextOmittedEdgeEvidence=(copied.match(/Edge relationship evidence \(each path leg remains separate\): candidates=(\d+); included=(\d+); omitted=(\d+)/)||[]).slice(1).map(Number);
  result.selections.push(snapshot);
  await page.screenshot({path:`${outDir}/${index+1}-${target.name}.png`,fullPage:false});
 }
 const endpoint=ledger.items.find(i=>i.family==='endpoint'&&i.owner==='ArticleController'&&i.handler==='findAll');
 await page.getByRole('searchbox',{name:'Search symbols'}).fill('GET /articles');
 await page.getByRole('list',{name:'Search results'}).getByRole('button').filter({hasText:endpoint.expectedNode.id}).click();
 const endpointText=await page.locator('aside[aria-label="Node details"]').innerText();
 result.endpoint={id:endpoint.expectedNode.id,selectedMatches:(await page.locator('.canvas-viewport').getAttribute('data-selected-nodes'))===endpoint.expectedNode.id,routeVisible:endpointText.includes('GET /articles'),controllerVisible:endpointText.includes('ArticleController'),handlerVisible:endpointText.includes('findAll'),declaredEndpointLabel:endpointText.includes('Declared endpoint')};
 result.requestsAfterImport=requests.slice(initialRequests);
 fs.writeFileSync(`${outDir}/summary.json`,JSON.stringify(result,null,2)+'\n');
 const counts=result.analysis.match(/In file: (\d+) nodes · (\d+) edges · (\d+) diagnostics/);
 const calls=result.analysis.match(/Examined call sites: (\d+) · Emitted call sites: (\d+) · Skipped call sites: (\d+)/);
 if(!counts||!calls) throw Error('Analysis counters are missing from UI');
 const actualSummary={url:result.url,graphSha256:result.graphSha256,analyzedAt:result.analyzedAt,canvas:result.canvas,
  requestsAfterImport:result.requestsAfterImport,consoleErrors:result.consoleErrors,
  method:'Local headless Playwright Chromium, 1440x900, actual browser File input and clipboard readback; source-first selections from source-v7',
  analysisCounts:{nodes:Number(counts[1]),edges:Number(counts[2]),diagnostics:Number(counts[3]),callsExamined:Number(calls[1]),callsEmitted:Number(calls[2]),callsSkipped:Number(calls[3])},
  selections:result.selections.map(({detailExcerpt,contextExcerpt,...rest})=>rest),endpoint:result.endpoint};
 const baseline=JSON.parse(fs.readFileSync(baselinePath,'utf8'));
 const differences=compareSummaries(comparableSummary(baseline,freshSnapshot),comparableSummary(actualSummary,freshSnapshot));
 if(differences.length) throw Error(`UI evidence differs from fixed baseline: ${differences.join(', ')}`);
 console.log(JSON.stringify({status:freshSnapshot?'WITHIN_STRUCTURAL_BASELINE':'WITHIN_FIXED_BASELINE',graphSha256:result.graphSha256,selections:result.selections.map(s=>({name:s.name,contextSha256:s.contextSha256})),requestsAfterImport:result.requestsAfterImport.length,consoleErrors:result.consoleErrors.length},null,2));
 }finally{await browser.close();}
}

if(require.main===module){
 if(process.argv[2]==='--compare'){
  try{
   const expected=JSON.parse(fs.readFileSync(process.argv[3],'utf8'));
   const actual=JSON.parse(fs.readFileSync(process.argv[4],'utf8'));
   const freshSnapshot=process.argv.includes('--fresh-snapshot');
   const differences=compareSummaries(comparableSummary(expected,freshSnapshot),comparableSummary(actual,freshSnapshot));
   if(differences.length) throw Error(`UI evidence differs from fixed baseline: ${differences.join(', ')}`);
  }catch(e){console.error(e);process.exitCode=1;}
 }else run().catch(e=>{console.error(e);process.exitCode=1});
}
module.exports={compareSummaries,comparableSummary};
