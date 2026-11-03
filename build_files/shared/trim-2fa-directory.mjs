// Makes system_files/shared/usr/share/amethystora/vault/2fa-directory.json from 2factorauth/twofactorauth: the
// Vault's list of services that offer two-factor sign-in, with where to set it up and where to recover (plans/vault-bmc/
// spec-formats.md, section 13). Not part of the image build: run by hand to move to a newer commit, after which the
// commit and the checksum in usr/share/licenses/2fa-directory/SOURCE move with it.
//
//   node build_files/shared/trim-2fa-directory.mjs TWOFACTORAUTH/entries OUT.json UPDATED-DATE LICENSE.md
//
// A row is [name, domain, documentation or 0, recovery or 0, [other domains]?], kept when the entry offers two-factor
// sign-in and has documentation or recovery, in the order of the names without regard to case.
import fs from 'node:fs';
import path from 'node:path';

const [folder, out, updated, licence] = process.argv.slice(2);
const rows = [];
for (const letter of fs.readdirSync(folder)) {
    for (const file of fs.readdirSync(path.join(folder, letter))) {
        const entry = JSON.parse(fs.readFileSync(path.join(folder, letter, file), 'utf8'));
        for (const [name, site] of Object.entries(entry)) {
            const docs = typeof site.documentation === 'string' && site.documentation ? site.documentation : 0;
            const recovery = typeof site.recovery === 'string' && site.recovery ? site.recovery : 0;
            if (!Array.isArray(site.tfa) || (!docs && !recovery)) continue;
            const row = [name, site.domain, docs, recovery];
            if (Array.isArray(site['additional-domains']) && site['additional-domains'].length) row.push(site['additional-domains']);
            rows.push(row);
        }
    }
}
rows.sort((a, b) => a[0].toLowerCase().localeCompare(b[0].toLowerCase(), 'en') || (a[0] < b[0] ? -1 : 1));
const text = fs.readFileSync(licence, 'utf8');
const mit = text.slice(text.indexOf('The MIT License (MIT)'), text.indexOf('# 2014-2020')).trim();
fs.writeFileSync(out, JSON.stringify({ source: 'https://2fa.directory', updated, license: mit, entries: rows }) + '\n');
console.log(`${rows.length} rows`);
