#!/usr/bin/env node
import { Command } from 'commander';
import { scanCommand } from './commands/scan';
import { checkCommand } from './commands/check';
import { updateCommand } from './commands/update';
import { listPathsCommand } from './commands/list-paths';
import { showCommand } from './commands/show';

const program = new Command();

program
  .name('treeupdt')
  .description('Keep your dependency tree fresh')
  .version('0.1.0');

program.addCommand(scanCommand);
program.addCommand(checkCommand);
program.addCommand(updateCommand);
program.addCommand(listPathsCommand);
program.addCommand(showCommand);

program.parse();