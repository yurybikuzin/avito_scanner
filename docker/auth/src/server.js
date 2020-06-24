const puppeteer = require('puppeteer-core');
const devices = require('puppeteer-core/DeviceDescriptors');
const iPhoneX = devices.devicesMap['iPhone X'];
const express = require('express');
const app = express();
const port = 3000;

app.get('/', async (req, res) => {
    let key = null;
    try {
        key = await getKey()
    } catch (err) {
        res.status(500).send(err.message);
        return;
    }
    if (key) {
        res.send(key);
    } else {
        res.status(404).send('Failed to get key');
    }
});

app.listen(port, () => {});

const reSkip = /(?:(?:securepubads|stats)\.g\.doubleclick\.net|sslwidget\.criteo\.com|static\.criteo\.net|(?:mc|an)\.yandex\.ru|yastatic\.net|ad\.mail\.ru|www\.google-analytics\.com|www\.googletagservices\.com|www.googletagmanager\.com|adservice\.google\.com|connect\.facebook\.net|cs\.avito.ru\/clickstream)/;
const reMatch = /api\/1\/rmp\/search\?key=([^&]+)/;

async function getKey() {
    const browser = await puppeteer.launch({
        executablePath: '/usr/bin/google-chrome',
        args: [
            '--no-sandbox',
            '--disable-setuid-sandbox',
            "--ignore-certificate-errors"
        ]
    });
    const page = await browser.newPage();
    await page.emulate(devices.devicesMap['iPhone X']);
    await page.setRequestInterception(true);
    let result = null;
    page.on('request', request => {
      const url = request.url();
      if (request.resourceType() === 'image' || reSkip.test(url)) {
          request.abort();
      } else {
          if (reMatch.test(url)) {
              result = url.match(reMatch)[1];
          }
          request.continue();
      }
    });
    await page.goto('https://m.avito.ru/moskva/avtomobili?s=104&user=1', {waitUntil: 'networkidle2'});
    return result
}
