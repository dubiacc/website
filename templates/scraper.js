const puppeteer = require('puppeteer');
const axios = require('axios');
const Papa = require('papaparse');
const fs = require('fs-extra');
const path = require('path');
const cheerio = require('cheerio');

// URLs of Google Sheets
const MAIN_SHEET_URL = 'https://docs.google.com/spreadsheets/d/12kg-wXZsFPgxObEO1mK5GluVNdBbTzzek15qnJ9EyhE/gviz/tq?tqx=out:csv&sheet=scrape&query=rows';
const ADDITIONAL_SHEETS = [
  'https://docs.google.com/spreadsheets/d/16wO1gTdilEcdqsWfJ0mZn-hgYD2MRJ1SCKSAHzcDj18/gviz/tq?tqx=out:csv&sheet=events&query=rows',
  'https://docs.google.com/spreadsheets/d/16wO1gTdilEcdqsWfJ0mZn-hgYD2MRJ1SCKSAHzcDj18/gviz/tq?tqx=out:csv&sheet=locations&query=rows',
  'https://docs.google.com/spreadsheets/d/16wO1gTdilEcdqsWfJ0mZn-hgYD2MRJ1SCKSAHzcDj18/gviz/tq?tqx=out:csv&sheet=religious&query=rows'
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

// Parse CSV data
function parseCSV(csvData) {
  console.log(`Parsing CSV data...`);
  const results = Papa.parse(csvData, {
    header: true,
    skipEmptyLines: true,
    dynamicTyping: true
  });
  console.log(`Parsed ${results.data.length} rows from CSV data`);
  return results.data;
}

// Fetch a website using Puppeteer with JavaScript support
async function fetchWithJavaScript(url, extractorFunction = '', index, total) {
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
  
  try {
    const page = await browser.newPage();
    await page.setDefaultNavigationTimeout(90000);
    await page.setDefaultTimeout(60000);
    
    console.log(`Navigating to ${url}...`);
    await page.goto(url, { waitUntil: 'networkidle2', timeout: 60000 });
    console.log(`Successfully loaded ${url}`);
    
    // Use page.content() to get the HTML with all JavaScript executed
    console.log(`Getting page content with JavaScript executed...`);
    let content = await page.content();
    console.log(`Successfully retrieved page content (${content.length} bytes)`);
    
    // Save raw content for debugging
    const rawFilename = `raw_content_${index}.html`;
    saveContent(rawFilename, content);
    console.log(`Saved raw content to ${rawFilename}`);
    
    // If there's an extractor function, apply it using cheerio
    if (extractorFunction && extractorFunction.trim() !== '') {
      console.log(`Applying extractor function...`);
      
      // Save the extractor function for debugging
      const extractorFilename = `extractor_function_${index}.js`;
      saveContent(extractorFilename, extractorFunction);
      
      try {
        const $ = cheerio.load(content);
        
        // Create a function from the extractor code
        const extractFunction = new Function('$', extractorFunction);
        const extractResult = extractFunction($);
        
        if (!extractResult) {
          throw new Error(`Extractor function did not return a result for ${url}`);
        }
        
        content = typeof extractResult === 'string' 
          ? extractResult 
          : JSON.stringify(extractResult, null, 2);
        
        console.log(`Extraction successful: ${content.substring(0, 100)}...`);
        
        // Save extracted content for debugging
        const extractedFilename = `extracted_content_${index}.txt`;
        saveContent(extractedFilename, content);
      } catch (extractorError) {
        console.error(`Error applying extractor function for ${url}:`, extractorError);
        
        // Save error details
        const errorFilename = `extractor_error_${index}.txt`;
        saveContent(errorFilename, `Error: ${extractorError.message}\n\nStack: ${extractorError.stack}`);
        
        // Exit with error
        throw extractorError;
      }
    }
    
    return content;
  } catch (error) {
    console.error(`Error fetching ${url} with JavaScript:`, error.message);
    
    // Save error details
    const errorFilename = `fetch_error_${index}.txt`;
    saveContent(errorFilename, `Error: ${error.message}\n\nStack: ${error.stack}`);
    
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

// Main function
async function main() {
  try {
    console.log("=== Web Scraper Started ===");
    // Create data directory if it doesn't exist
    fs.ensureDirSync(dataDir);
    
    // Fetch and save the main CSV
    console.log("\n=== Fetching Main Sheet ===");
    const mainCsvData = await fetchCSV(MAIN_SHEET_URL);
    if (!mainCsvData) throw new Error("Failed to fetch main CSV");
    
    const mainCsvPath = saveContent('main_sheet.csv', mainCsvData);
    const urlsToScrape = parseCSV(mainCsvData);
    
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
    
    // Scrape URLs from the main CSV
    console.log("\n=== Scraping URLs ===");
    const scrapedContents = [];
    const validUrls = urlsToScrape.filter(row => row.URL);
    const totalUrls = validUrls.length;
    
    for (const [index, row] of validUrls.entries()) {
      console.log(`\nProcessing URL ${index+1}/${totalUrls} (${Math.round((index+1)/totalUrls*100)}%)`);
      console.log(`URL: ${row.URL}`);
      
      try {
        const content = await fetchWithJavaScript(row.URL, row.ExtractorFunction || '', index, totalUrls);
        const filename = `scraped_content_${index}.txt`;
        const contentPath = saveContent(filename, content);
        scrapedContents.push({ path: contentPath, name: filename, url: row.URL });
      } catch (error) {
        console.error(`Failed to process URL ${row.URL}:`, error.message);
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