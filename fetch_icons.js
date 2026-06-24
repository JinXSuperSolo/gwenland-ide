const https = require('https');
const fs = require('fs');

const icons = ['rust', 'toml', 'html', 'markdown', 'json', 'lock', 'git', 'file'];
const baseUrl = 'https://raw.githubusercontent.com/material-extensions/vscode-material-icon-theme/main/icons/';

const fetchIcon = (name) => {
    return new Promise((resolve, reject) => {
        https.get(`${baseUrl}${name}.svg`, (res) => {
            let data = '';
            res.on('data', chunk => data += chunk);
            res.on('end', () => resolve({ name, data: data.trim() }));
        }).on('error', reject);
    });
};

Promise.all(icons.map(fetchIcon)).then(results => {
    const iconObj = {};
    results.forEach(r => {
        iconObj[r.name] = r.data;
    });
    fs.writeFileSync('icons.json', JSON.stringify(iconObj, null, 2));
    console.log('Icons fetched successfully.');
}).catch(console.error);
