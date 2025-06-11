const puppeteer = require('puppeteer');
const axios = require('axios');
const fs = require('fs-extra');
const path = require('path');
const { JWT } = require('google-auth-library');
const { GoogleSpreadsheet } = require('google-spreadsheet');

// URLs to scrape - easily visible at the top
const URLS_TO_SCRAPE = [
  'https://apotresdejesusetdemarie.fr/evenements/',
  'https://abbe-pivert.com/ministere-dominical/',
  'https://sspxmc.com/mass-schedule/',
];

const SPREADSHEET_ID = "16wO1gTdilEcdqsWfJ0mZn-hgYD2MRJ1SCKSAHzcDj18";
const LOCATIONS_SHEET = 'locations';
const RELIGIOUS_SHEET = 'religious';
const EVENTS_SHEET = 'events';

// URLs of additional Google Sheets for reference data
const ADDITIONAL_SHEETS = [
  `https://docs.google.com/spreadsheets/d/${SPREADSHEET_ID}/gviz/tq?tqx=out:csv&sheet=${EVENTS_SHEET}`,
  `https://docs.google.com/spreadsheets/d/${SPREADSHEET_ID}/gviz/tq?tqx=out:csv&sheet=${LOCATIONS_SHEET}`,
  `https://docs.google.com/spreadsheets/d/${SPREADSHEET_ID}/gviz/tq?tqx=out:csv&sheet=${RELIGIOUS_SHEET}`
];

// Create data directory
const dataDir = path.join(process.cwd(), 'data');
fs.ensureDirSync(dataDir);

// Fetch a CSV file
async function fetchCSV(url) {
  try {
    console.log(`Fetching CSV from ${url}...`);
    const response = await axios.get(url);
    console.log(`Successfully fetched CSV from ${url}`);
    return response.data;
  } catch (error) {
    console.error(`Error fetching CSV from ${url}:`, error.message);
    return null;
  }
}

// Clean HTML content using browser-native tools
async function cleanHtml(page) {
  return await page.evaluate(() => {
    const parser = new DOMParser();
    const doc = parser.parseFromString(document.documentElement.outerHTML, 'text/html');
    
    const scripts = doc.querySelectorAll('script:not([type="application/ld+json"])');
    scripts.forEach(script => script.remove());
    
    const styles = doc.querySelectorAll('style');
    styles.forEach(style => style.remove());
    
    const links = doc.querySelectorAll('link[rel="stylesheet"]');
    links.forEach(link => link.remove());
    
    const allElements = doc.querySelectorAll('*');
    allElements.forEach(el => {
      el.removeAttribute('style');
      el.removeAttribute('class');
      for (const attr of [...el.attributes]) {
        if (attr.name.startsWith('data-')) {
          el.removeAttribute(attr.name);
        }
      }
    });
    
    return doc.documentElement.outerHTML;
  });
}

// Fetch a website using Puppeteer with JavaScript support
async function fetchWithJavaScript(url, index, total) {
  console.log(`[${index+1}/${total}] Fetching with JavaScript: ${url}`);
  const browser = await puppeteer.launch({
    executablePath: process.env.CHROME_PATH || undefined,
    args: [
      '--no-sandbox',
      '--disable-setuid-sandbox',
      '--disable-dev-shm-usage',
      '--disable-accelerated-2d-canvas',
      '--no-first-run',
      '--no-zygote',
      '--disable-gpu',
      '--headless',
      '--disable-notifications',
      '--disable-extensions',
      '--disable-popup-blocking'
    ],
    headless: true,
    timeout: 60000
  });
  
  let rawHtml = '';
  
  try {
    const page = await browser.newPage();
    await page.setDefaultNavigationTimeout(90000);
    await page.setDefaultTimeout(60000);
    
    console.log(`Navigating to ${url}...`);
    await page.goto(url, { waitUntil: 'networkidle2', timeout: 60000 });
    console.log(`Successfully loaded ${url}`);
    
    console.log(`Getting raw HTML with JavaScript executed...`);
    rawHtml = await page.content();
    const rawFilename = `raw_content_${index}.html`;
    saveContent(rawFilename, rawHtml);
    console.log(`Saved raw content to ${rawFilename} (${rawHtml.length} bytes)`);
    
    console.log(`Cleaning HTML content...`);
    try {
      const cleanedHtml = await cleanHtml(page);
      
      console.log(`Successfully cleaned HTML content (${cleanedHtml.length} bytes)`);
      
      const cleanedFilename = `cleaned_content_${index}.html`;
      saveContent(cleanedFilename, cleanedHtml);
      console.log(`Saved cleaned content to ${cleanedFilename}`);
      
      return cleanedHtml;
    } catch (cleaningError) {
      console.warn(`Warning: HTML cleaning failed, using raw HTML instead: ${cleaningError.message}`);
      saveContent(`cleaning_error_${index}.txt`, `${cleaningError.message}\n\n${cleaningError.stack}`);
      
      console.log(`Falling back to raw HTML content`);
      return rawHtml;
    }
  } catch (error) {
    console.error(`Error fetching ${url} with JavaScript:`, error.message);
    
    const errorFilename = `fetch_error_${index}.txt`;
    saveContent(errorFilename, `Error: ${error.message}\n\nStack: ${error.stack}`);
    
    if (rawHtml) {
      console.log(`Despite error, returning available raw HTML (${rawHtml.length} bytes)`);
      return rawHtml;
    }
    
    throw error;
  } finally {
    await browser.close();
    console.log(`Browser closed for ${url}`);
  }
}

// Save content to a file
function saveContent(filename, content) {
  const filePath = path.join(dataDir, filename);
  fs.writeFileSync(filePath, content);
  console.log(`Saved to ${filePath}`);
  return filePath;
}

// Convert file to base64
function fileToBase64(filePath) {
  console.log(`Converting ${filePath} to base64...`);
  const content = fs.readFileSync(filePath);
  const base64Content = content.toString('base64');
  console.log(`Converted ${filePath} to base64 (${base64Content.length} chars)`);
  return base64Content;
}

// Process Gemini API response
function processGeminiResponse(responseData) {
  console.log(`Processing Gemini API response...`);
  
  const finishReason = responseData?.candidates?.[0]?.finishReason;
  if (finishReason === 'MAX_TOKENS') {
    console.warn('⚠️ WARNING: Response truncated (MAX_TOKENS) - JSON may be incomplete!');
    saveContent('gemini_truncation_warning.txt', 'Response was truncated due to MAX_TOKENS limit');
  }
  
  if (!responseData?.candidates?.[0]?.content?.parts?.[0]?.text) {
    throw new Error('Invalid Gemini API response format');
  }
  
  let responseText = responseData.candidates[0].content.parts[0].text;
  
  saveContent('gemini_raw_response.txt', responseText);
  
  if (responseText.startsWith('```json')) {
    console.log('Removing markdown code block formatting...');
    responseText = responseText.replace(/^```json\n/, '').replace(/\n```$/, '');
  } else if (responseText.startsWith('```')) {
    responseText = responseText.replace(/^```\n/, '').replace(/\n```$/, '');
  }
  
  try {
    const jsonData = JSON.parse(responseText);
    console.log('Successfully parsed JSON from Gemini response');
    return JSON.stringify(jsonData, null, 2);
  } catch (error) {
    console.error('Failed to parse JSON from Gemini response:', error.message);
    saveContent('json_parse_error.txt', `${error.message}\n\nPartial text:\n${responseText.substring(0, 1000)}...`);
    
    return responseText;
  }
}

// Parse and validate service account credentials
function parseServiceAccountCredentials(credentialsJson) {
  try {
    const credentials = JSON.parse(credentialsJson);
    
    const requiredFields = ['client_email', 'private_key', 'type'];
    for (const field of requiredFields) {
      if (!credentials[field]) 
        throw new Error(`Missing required field in service account credentials: ${field}`);
    }
    
    if (credentials.type !== 'service_account')
      throw new Error(`Invalid credential type: expected 'service_account', got '${credentials.type}'`);
    
    return credentials;
  } catch (error) {
    if (error instanceof SyntaxError)
      throw new Error(`Invalid JSON format in service account credentials: ${error.message}`);
    throw error;
  }
}

// Update Google Spreadsheet with new data
async function updateSpreadsheet(jsonData) {
  if (!process.env.GOOGLE_DOCS_SERVICE_ACCOUNT) {
    console.error('Missing Google authentication credentials. Skipping spreadsheet update.');
    return false;
  }

  console.log('\n=== Updating Google Spreadsheet ===');
  
  try {
    const credentials = parseServiceAccountCredentials(process.env.GOOGLE_DOCS_SERVICE_ACCOUNT);
    
    const serviceAccountAuth = new JWT({
      email: credentials.client_email,
      key: credentials.private_key,
      scopes: ['https://www.googleapis.com/auth/spreadsheets'],
    });
    
    const doc = new GoogleSpreadsheet(SPREADSHEET_ID, serviceAccountAuth);
    
    await doc.loadInfo();
    console.log(`Connected to spreadsheet: ${doc.title}`);
    
    if (jsonData.newlocations?.length > 0) 
      await appendRows(doc, LOCATIONS_SHEET, jsonData.newlocations);
    
    if (jsonData.newreligious?.length > 0) 
      await appendRows(doc, RELIGIOUS_SHEET, jsonData.newreligious);
    
    if (jsonData.newevents?.length > 0) 
      await appendRows(doc, EVENTS_SHEET, jsonData.newevents);
    
    console.log('Spreadsheet update completed successfully');
    return true;
  } catch (error) {
    console.error('Error updating spreadsheet:', error.message);
    saveContent('spreadsheet_update_error.txt', `${error.message}\n\n${error.stack}`);
    return false;
  }
}

// Append rows to a specific sheet
async function appendRows(doc, sheetName, data) {
  const sheet = doc.sheetsByTitle[sheetName];
  if (!sheet) {
    throw new Error(`Sheet '${sheetName}' not found in spreadsheet`);
  }
  
  console.log(`Adding ${data.length} rows to ${sheetName}`);
  await sheet.addRows(data);
  console.log(`Successfully added ${data.length} rows to ${sheetName}`);
}

// Main function
async function main() {
  // Define variables in a higher scope to be accessible in catch blocks
  let allFiles = []; 
  let promptText = '';

  try {
    console.log("=== Web Scraper Started ===");
    fs.ensureDirSync(dataDir);
    
    console.log("\n=== Fetching Additional Sheets ===");
    const additionalCsvs = [];
    for (let i = 0; i < ADDITIONAL_SHEETS.length; i++) {
      const url = ADDITIONAL_SHEETS[i];
      const filename = `additional_sheet_${i + 1}.csv`;
      const csvData = await fetchCSV(url);
      if (csvData) {
        const csvPath = saveContent(filename, csvData);
        additionalCsvs.push({ path: csvPath, name: filename });
      }
    }
    
    console.log("\n=== Scraping URLs ===");
    const scrapedContents = [];
    const totalUrls = URLS_TO_SCRAPE.length;
    
    for (const [index, url] of URLS_TO_SCRAPE.entries()) {
      console.log(`\nProcessing URL ${index+1}/${totalUrls} (${Math.round((index+1)/totalUrls*100)}%)`);
      console.log(`URL: ${url}`);
      
      try {
        const content = await fetchWithJavaScript(url, index, totalUrls);
        const filename = `scraped_content_${index}.html`;
        const contentPath = saveContent(filename, content);
        scrapedContents.push({ path: contentPath, name: filename, url: url });
      } catch (error) {
        console.error(`Failed to process URL ${url}:`, error.message);
      }
    }
    
    console.log("\n=== Preparing Gemini API Request ===");
    
    allFiles = [...additionalCsvs, ...scrapedContents];
    console.log(`Including ${allFiles.length} files in the request`);
    
    promptText = `Here is the result of a scraped website, as well as the existing data of locations and religious.
Match all the location IDs and religious IDs if you can, generate new event IDs if the event doesn't already exist in the table, then output only the rows that need to be added, as a JSON with the keys "newlocations", "newreligious" and "newevents". The data will later on be used to update the input table.

IMPORTANT: Return your response as valid, complete JSON without any markdown formatting.`;
    
    const requestBody = {
      contents: [{
        parts: [
          { text: promptText },
          ...allFiles.map(file => ({
            inlineData: {
              mimeType: "text/plain",
              data: fileToBase64(file.path)
            }
          }))
        ]
      }]
    };
    
    saveContent('gemini_request.json', JSON.stringify(requestBody, null, 2));
    
    if (!process.env.GEMINI_API_KEY) {
      console.error("ERROR: GEMINI_API_KEY environment variable not set.");
      console.error("Please add your Gemini API key as a GitHub Secret named GEMINI_API_KEY.");
      process.exit(1);
    }
    
    console.log("\n=== Sending Request to Gemini API ===");
    try {
      const geminiUrl = `https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-pro-latest:generateContent?key=${process.env.GEMINI_API_KEY}`;
      console.log(`Using model: gemini-1.5-pro-latest`);
      
      const response = await axios.post(
        geminiUrl,
        requestBody,
        {
          headers: { 'Content-Type': 'application/json' }
        }
      );
      
      saveContent('gemini_full_response.json', JSON.stringify(response.data, null, 2));
      
      const processedResponse = processGeminiResponse(response.data);
      
      saveContent('gemini_processed_data.json', processedResponse);
      console.log("\n=== Successfully processed data with Gemini API ===");
      
      if (processedResponse) {
        try {
          const jsonData = JSON.parse(processedResponse);
          await updateSpreadsheet(jsonData);
        } catch (error) {
          console.error("Error parsing JSON for spreadsheet update:", error.message);
          saveContent('json_parse_error_for_update.txt', error.message);
        }
      }
      
    } catch (error) {
      console.error("Error calling Gemini API:", error.message);
      if (error.response) {
        console.error("API Response:", JSON.stringify(error.response.data, null, 2));
        saveContent('gemini_api_error.json', JSON.stringify(error.response.data, null, 2));
      }

      // --- MODIFICATION: Save data for manual run on failure ---
      console.log("\n[FAILURE] Gemini API call failed. Saving a manifest file (error.json) for manual processing.");
      try {
        const manualRunData = {
          prompt: promptText,
          files_to_upload: allFiles.map(file => file.name),
          note: "The files listed above are available in the 'scraped-data' artifact from the failed GitHub Actions run. Please upload them to the Gemini Web UI along with the prompt to manually complete the process."
        };
        const errorFilePath = saveContent('error.json', JSON.stringify(manualRunData, null, 2));
        console.log(`Successfully saved manual run manifest to ${errorFilePath}.`);
      } catch (saveError) {
        console.error("Could not save the manual run manifest (error.json):", saveError.message);
      }
      // --- END MODIFICATION ---

      process.exit(1);
    }
  } catch (error) {
    console.error("\n=== FATAL ERROR ===");
    console.error(error);
    saveContent('fatal_error.txt', `${error.message}\n\n${error.stack}`);
    
    if (allFiles.length > 0 && promptText) {
        console.log("\n[FAILURE] Fatal error occurred. Attempting to save manifest file (error.json) for manual processing.");
        try {
            const manualRunData = {
              prompt: promptText,
              files_to_upload: allFiles.map(file => file.name),
              note: "The files listed above are available in the 'scraped-data' artifact from the failed GitHub Actions run. Please upload them to the Gemini Web UI along with the prompt to manually complete the process."
            };
            saveContent('error.json', JSON.stringify(manualRunData, null, 2));
        } catch (saveError) {
            console.error("Could not save the manual run manifest (error.json):", saveError.message);
        }
    }
    
    process.exit(1);
  }
}

// Run the main function
main();