import { chromium } from '@playwright/test';

const SHELL = 'http://127.0.0.1:5173';
const OUT = '/tmp/claude-1000/-home-user-code-rust-lb/81ee1652-926f-4099-8f00-1cc4fa189c95/scratchpad';
const shot = async (page, name) => { await page.screenshot({ path: `${OUT}/${name}.png`, fullPage: true }); console.log('shot:', name); };

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1400, height: 900 } });
const errs = [];
page.on('console', m => { if (m.type() === 'error') errs.push('[console] ' + m.text()); });
page.on('pageerror', e => errs.push('[pageerror] ' + e.message));

try {
  await page.goto(SHELL, { waitUntil: 'networkidle' });
  await shot(page, '01-loaded');

  // login
  await page.getByLabel('identity').fill('user:ada');
  await page.getByLabel('workspace').fill('acme');
  await page.getByLabel('sign in').click();
  await page.waitForTimeout(1500);
  await shot(page, '02-loggedin');

  // go to dashboards
  await page.getByRole('button', { name: 'Dashboards', exact: true }).click();
  await page.waitForTimeout(1000);
  await shot(page, '03-dashboards');

  // create a fresh dashboard
  const titleInput = page.getByLabel('new dashboard title');
  if (await titleInput.isVisible().catch(() => false)) {
    await titleInput.fill('AI Widget Explore');
    await page.getByLabel('create dashboard').click();
    await page.waitForTimeout(1500);
  }
  await shot(page, '04-created');

  // add panel
  const addPanel = page.getByLabel('add panel');
  console.log('add panel visible:', await addPanel.isVisible().catch(() => false));
  await addPanel.click();
  await page.waitForTimeout(1000);
  await shot(page, '05-editor-open');

  // pick AI widget viz
  const aiViz = page.getByLabel('viz genui');
  console.log('viz genui visible:', await aiViz.isVisible().catch(() => false));
  console.log('viz genui disabled:', await aiViz.getAttribute('aria-disabled').catch(() => '?'));
  await aiViz.click();
  await page.waitForTimeout(500);
  await shot(page, '06-genui-picked');

  // open Panel options tab
  const optTab = page.getByRole('button', { name: 'Panel options' }).or(page.getByText('Panel options', { exact: true }));
  console.log('panel options visible:', await optTab.first().isVisible().catch(() => false));
  await optTab.first().click();
  await page.waitForTimeout(500);
  await shot(page, '07-options-tab');

  // type a prompt and generate
  const promptBox = page.getByLabel('widget prompt');
  console.log('prompt box visible:', await promptBox.isVisible().catch(() => false));
  await promptBox.fill('a stat tile showing the number 42 labeled "Open alerts", red when above 10');
  await shot(page, '08-prompt-typed');

  await page.getByLabel('generate widget').click();
  console.log('clicked generate, waiting for agent...');
  // wait up to 90s for either preview or error
  for (let i = 0; i < 90; i++) {
    await page.waitForTimeout(1000);
    const gen = page.getByLabel('generate widget');
    const label = await gen.textContent().catch(() => '');
    if (!/Generating/.test(label)) { console.log('generate done at ~', i, 's; button:', label); break; }
  }
  await shot(page, '09-after-generate');

  // capture any error text
  const alert = page.locator('[role="alert"]');
  if (await alert.count()) console.log('ALERT:', await alert.first().textContent());

  // accept
  const acceptBtn = page.getByLabel('accept widget');
  console.log('accept disabled:', await acceptBtn.isDisabled().catch(() => '?'));
  if (!(await acceptBtn.isDisabled().catch(() => true))) {
    await acceptBtn.click();
    await page.waitForTimeout(800);
    await shot(page, '10-accepted');
  }

  // save
  const saveBtn = page.getByLabel('save panel');
  if (await saveBtn.isVisible().catch(() => false)) {
    await saveBtn.click();
    await page.waitForTimeout(1500);
    await shot(page, '11-saved');
  }
} catch (e) {
  console.log('SCRIPT ERROR:', e.message);
  await shot(page, 'zz-error');
} finally {
  console.log('\n=== browser errors ===');
  console.log(errs.slice(0, 40).join('\n') || '(none)');
  await browser.close();
}
