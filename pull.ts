// ---------------------------------------------------------------------------
// Run: npx tsx scrape_vwrl.ts
// ---------------------------------------------------------------------------

import { chromium } from 'playwright';
import { writeFile } from 'fs/promises';

(async () => {
  // 1. Launch Browser
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({
    userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
    viewport: { width: 1280, height: 720 },
  });
  const page = await context.newPage();

  // List of symbols to scrape
  const symbols = ['EURONEXT-VWRL', 'TSLA', 'GOLD'];
  let fileContent = '';

  console.log('Starting batch scrape...');

  for (const symbol of symbols) {
    const url = `https://www.tradingview.com/symbols/${symbol}/`;
    process.stdout.write(`Scraping ${symbol}... `); // Print without newline

    try {
      // 2. Navigate
      await page.goto(url, { waitUntil: 'domcontentloaded' });

      // 3. Define Selectors
      const priceSelector = '.js-symbol-last, span[class^="last-"]';

      // 4. Wait for price
      const priceElement = page.locator(priceSelector).first();
      await priceElement.waitFor({ state: 'visible', timeout: 8000 });

      // 5. Extract and Format
      const priceText = await priceElement.textContent();
      let finalPrice = priceText?.trim() || 'N/A';

      // Remove thousand separators (commas)
      finalPrice = finalPrice.replace(/,/g, '');

      // Drop decimal points (keep only integer part)
      finalPrice = finalPrice.split('.')[0];
      
      console.log(`Done (${finalPrice})`);

      // Rename EURONEXT-VWRL to VWRL for the file
      const displaySymbol = symbol === 'EURONEXT-VWRL' ? 'VWRL' : symbol;
      
      // Determine currency symbol: € for VWRL, $ for others
      const currency = symbol === 'EURONEXT-VWRL' ? '$' : '$';

      fileContent += `${displaySymbol}:${finalPrice}${currency}\n`;

    } catch (error) {
      console.log('Failed');
      console.error(`  Error scraping ${symbol}:`, error instanceof Error ? error.message : error);
      fileContent += `${symbol}: ERROR\n`;
    }
  }

  // 6. Write to file
  await browser.close();
  
  try {
    await writeFile('prices.txt', fileContent, 'utf-8');
    console.log('------------------------------------------------');
    console.log('Successfully wrote to prices.txt');
    console.log('------------------------------------------------');
  } catch (err) {
    console.error('Error writing file:', err);
  }
})();
