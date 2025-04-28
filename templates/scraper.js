const puppeteer = require('puppeteer');
const axios = require('axios');
const fs = require('fs-extra');
const path = require('path');
const { JWT } = require('google-auth-library');

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
  `https://docs.google.com/spreadsheets/d/${SPREADSHEET_ID}/gviz/tq?tqx=out:csv&sheet=${EVENTS_SHEET}&query=rows`,
  `https://docs.google.com/spreadsheets/d/${SPREADSHEET_ID}/gviz/tq?tqx=out:csv&sheet=${LOCATIONS_SHEET}&query=rows`,
  `https://docs.google.com/spreadsheets/d/${SPREADSHEET_ID}/gviz/tq?tqx=out:csv&sheet=${RELIGIOUS_SHEET}&query=rows`
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
    // Create a new document to work with
    const parser = new DOMParser();
    const doc = parser.parseFromString(document.documentElement.outerHTML, 'text/html');
    
    // Remove all scripts EXCEPT ld+json
    const scripts = doc.querySelectorAll('script:not([type="application/ld+json"])');
    scripts.forEach(script => script.remove());
    
    // Remove all style tags
    const styles = doc.querySelectorAll('style');
    styles.forEach(style => style.remove());
    
    // Remove all link tags (external CSS)
    const links = doc.querySelectorAll('link[rel="stylesheet"]');
    links.forEach(link => link.remove());
    
    // Remove all inline styles and class attributes from elements
    const allElements = doc.querySelectorAll('*');
    allElements.forEach(el => {
      el.removeAttribute('style');
      el.removeAttribute('class');
      // Remove data attributes
      for (const attr of [...el.attributes]) {
        if (attr.name.startsWith('data-')) {
          el.removeAttribute(attr.name);
        }
      }
    });
    
    // Return the cleaned HTML
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
    
    // Save raw content for debugging
    console.log(`Getting raw HTML with JavaScript executed...`);
    rawHtml = await page.content();
    const rawFilename = `raw_content_${index}.html`;
    saveContent(rawFilename, rawHtml);
    console.log(`Saved raw content to ${rawFilename} (${rawHtml.length} bytes)`);
    
    // Clean the HTML
    console.log(`Cleaning HTML content...`);
    try {
      const cleanedHtml = await cleanHtml(page);
      
      console.log(`Successfully cleaned HTML content (${cleanedHtml.length} bytes)`);
      
      // Save cleaned content
      const cleanedFilename = `cleaned_content_${index}.html`;
      saveContent(cleanedFilename, cleanedHtml);
      console.log(`Saved cleaned content to ${cleanedFilename}`);
      
      return cleanedHtml;
    } catch (cleaningError) {
      console.warn(`Warning: HTML cleaning failed, using raw HTML instead: ${cleaningError.message}`);
      saveContent(`cleaning_error_${index}.txt`, `${cleaningError.message}\n\n${cleaningError.stack}`);
      
      // Return raw HTML if cleaning fails
      console.log(`Falling back to raw HTML content`);
      return rawHtml;
    }
  } catch (error) {
    console.error(`Error fetching ${url} with JavaScript:`, error.message);
    
    // Save error details
    const errorFilename = `fetch_error_${index}.txt`;
    saveContent(errorFilename, `Error: ${error.message}\n\nStack: ${error.stack}`);
    
    // If we have raw HTML even though an error occurred, return it
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
  
  // Check for completion status
  const finishReason = responseData?.candidates?.[0]?.finishReason;
  if (finishReason === 'MAX_TOKENS') {
    console.warn('⚠️ WARNING: Response truncated (MAX_TOKENS) - JSON may be incomplete!');
    saveContent('gemini_truncation_warning.txt', 'Response was truncated due to MAX_TOKENS limit');
  }
  
  // Extract text from response
  if (!responseData?.candidates?.[0]?.content?.parts?.[0]?.text) {
    throw new Error('Invalid Gemini API response format');
  }
  
  let responseText = responseData.candidates[0].content.parts[0].text;
  
  // Save raw response text for debugging
  saveContent('gemini_raw_response.txt', responseText);
  
  // Clean markdown formatting if present
  if (responseText.startsWith('```json')) {
    console.log('Removing markdown code block formatting...');
    responseText = responseText.replace(/^```json\n/, '').replace(/\n```$/, '');
  } else if (responseText.startsWith('```')) {
    responseText = responseText.replace(/^```\n/, '').replace(/\n```$/, '');
  }
  
  // Try to parse as JSON
  try {
    const jsonData = JSON.parse(responseText);
    console.log('Successfully parsed JSON from Gemini response');
    return JSON.stringify(jsonData, null, 2);
  } catch (error) {
    console.error('Failed to parse JSON from Gemini response:', error.message);
    saveContent('json_parse_error.txt', `${error.message}\n\nPartial text:\n${responseText.substring(0, 1000)}...`);
    
    // Return cleaned text anyway
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
  
  // Don't modify/clean the column names - keep them as is, including question marks
  await sheet.addRows(data);
  console.log(`Successfully added ${data.length} rows to ${sheetName}`);
}

// Main function
async function main() {
  try {
    console.log("=== Web Scraper Started ===");
    // Create data directory if it doesn't exist
    fs.ensureDirSync(dataDir);
    
    // Fetch additional CSVs
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
    
    // Scrape URLs from the hardcoded list
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
        // Continue with next URL instead of failing completely
      }
    }
    
    // Prepare the request to Gemini API
    console.log("\n=== Preparing Gemini API Request ===");
    
    // List of all files to attach (CSV files + scraped contents)
    const allFiles = [...additionalCsvs, ...scrapedContents];
    console.log(`Including ${allFiles.length} files in the request`);
    
    // Create prompt
    const promptText = `Here is the result of a scraped website, as well as the existing data of locations and religious.
Match all the location IDs and religious IDs if you can, generate new event IDs if the event doesn't already exist in the table, then output only the rows that need to be added, as a JSON with the keys "newlocations", "newreligious" and "newevents". The data will later on be used to update the input table.

IMPORTANT: Return your response as valid, complete JSON without any markdown formatting.`;
    
    // Create request body
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
    
    // Save the request body for debugging
    saveContent('gemini_request.json', JSON.stringify(requestBody, null, 2));
    
    // Check if GEMINI_API_KEY is set
    if (!process.env.GEMINI_API_KEY) {
      console.error("ERROR: GEMINI_API_KEY environment variable not set.");
      console.error("Please add your Gemini API key as a GitHub Secret named GEMINI_API_KEY.");
      console.error("Go to your repository's Settings → Secrets and variables → Actions to add it.");
      process.exit(1);
    }
    
    // Make the request to Gemini API
    console.log("\n=== Sending Request to Gemini API ===");
    try {
      // Use the new model
      const geminiUrl = `https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-pro-exp-03-25:generateContent?key=${process.env.GEMINI_API_KEY}`;
      console.log(`Using model: gemini-2.5-pro-exp-03-25`);
      
      const response = await axios.post(
        geminiUrl,
        requestBody,
        {
          headers: {
            'Content-Type': 'application/json'
          }
        }
      );
      
      // Save full response for debugging
      saveContent('gemini_full_response.json', JSON.stringify(response.data, null, 2));
      
      // Process the response
      const processedResponse = processGeminiResponse(response.data);
      
      // Save processed data
      saveContent('gemini_processed_data.json', processedResponse);
      console.log("\n=== Successfully processed data with Gemini API ===");
      
      // Update Google Spreadsheet with processed data
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
      process.exit(1);
    }
  } catch (error) {
    console.error("\n=== FATAL ERROR ===");
    console.error(error);
    saveContent('fatal_error.txt', `${error.message}\n\n${error.stack}`);
    process.exit(1);
  }
}

// Run the main function
main();