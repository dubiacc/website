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
    const response = await axios.get(url);
    return response.data;
  } catch (error) {
    console.error(`Error fetching CSV from ${url}:`, error.message);
    return null;
  }
}

// Parse CSV data
function parseCSV(csvData) {
  const results = Papa.parse(csvData, {
    header: true,
    skipEmptyLines: true,
    dynamicTyping: true
  });
  return results.data;
}

// Load Puppeteer config
const puppeteerConfig = require('./puppeteer-config');

// Fetch a website using Puppeteer with JavaScript support
async function fetchWithJavaScript(url, extractorFunction = '') {
  console.log(`Fetching with JavaScript: ${url}`);
  const browser = await puppeteer.launch(puppeteerConfig.launch);
  
  try {
    const page = await browser.newPage();
    await page.goto(url, { waitUntil: 'networkidle2', timeout: 60000 });
    
    // Use page.content() to get the HTML with all JavaScript executed
    let content = await page.content();
    
    // If there's an extractor function, apply it using cheerio
    if (extractorFunction && extractorFunction.trim() !== '') {
      try {
        const $ = cheerio.load(content);
        // Using eval here for the extractorFunction - be careful with this in production
        // The extractorFunction should return a modified HTML or data object
        const extractResult = eval(`(function($) { ${extractorFunction} })`)(cheerio.load(content));
        if (extractResult) {
          content = typeof extractResult === 'string' ? extractResult : JSON.stringify(extractResult);
        }
      } catch (extractorError) {
        console.error(`Error applying extractor function for ${url}:`, extractorError);
      }
    }
    
    return content;
  } catch (error) {
    console.error(`Error fetching ${url} with JavaScript:`, error.message);
    return null;
  } finally {
    await browser.close();
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
  const content = fs.readFileSync(filePath);
  return content.toString('base64');
}

// Main function
async function main() {
  try {
    // Create data directory if it doesn't exist
    fs.ensureDirSync(dataDir);
    
    // Fetch and save the main CSV
    console.log("Fetching main CSV...");
    const mainCsvData = await fetchCSV(MAIN_SHEET_URL);
    if (!mainCsvData) throw new Error("Failed to fetch main CSV");
    
    const mainCsvPath = saveContent('main_sheet.csv', mainCsvData);
    const urlsToScrape = parseCSV(mainCsvData);
    
    // Fetch additional CSVs
    console.log("Fetching additional CSVs...");
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
    console.log("Scraping URLs...");
    const scrapedContents = [];
    for (const [index, row] of urlsToScrape.entries()) {
      if (!row.URL) continue;
      
      const content = await fetchWithJavaScript(row.URL, row.ExtractorFunction || '');
      if (content) {
        const filename = `scraped_content_${index}.txt`;
        const contentPath = saveContent(filename, content);
        scrapedContents.push({ path: contentPath, name: filename, url: row.URL });
      }
    }
    
    // Prepare the request to Gemini API
    console.log("Preparing Gemini API request...");
    
    // List of all files to attach (CSV files + scraped contents)
    const allFiles = [...additionalCsvs, ...scrapedContents];
    
    // Create prompt
    const promptText = `Here is the result of a scraped website, as well as the existing data of locations and religious.
Match all the location IDs and religious IDs if you can, generate new event IDs if the event doesn't already exist in the table, then output only the rows that need to be added, as a JSON with the keys "newlocations", "newreligious" and "newevents". The data will later on be used to update the input table.`;
    
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
    console.log("Sending request to Gemini API...");
    try {
      const response = await axios.post(
        `https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent?key=${process.env.GEMINI_API_KEY}`,
        requestBody,
        {
          headers: {
            'Content-Type': 'application/json'
          }
        }
      );
      
      // Save the response
      saveContent('gemini_response.json', JSON.stringify(response.data, null, 2));
      
      // Extract and save just the response text
      if (response.data?.candidates?.[0]?.content?.parts?.[0]?.text) {
        const responseText = response.data.candidates[0].content.parts[0].text;
        saveContent('gemini_processed_data.json', responseText);
        console.log("Successfully processed data with Gemini API");
      }
    } catch (error) {
      console.error("Error calling Gemini API:", error.message);
      if (error.response) {
        console.error("API Response:", JSON.stringify(error.response.data, null, 2));
      }
    }
    
  } catch (error) {
    console.error("Main error:", error);
    process.exit(1);
  }
}

// Run the main function
main();